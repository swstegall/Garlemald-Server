#!/usr/bin/env python3
"""Convert Project Meteor MySQL dumps into SQLite-flavored migration files
for Garlemald.

Reads every `.sql` file under `$INPUT_DIR` (default
`../project-meteor-mirror/Data/sql`) and writes a corresponding migration
file to `$OUTPUT_DIR` (default `common/sql/seed`) with names like
`NNN_<table>.sql`. Each output file contains a SQLite-flavoured
`CREATE TABLE IF NOT EXISTS` (harmless no-op for tables Garlemald's
`schema.sql` already creates), followed by `INSERT OR IGNORE INTO`
statements with explicit column lists so Garlemald-side column additions
don't break positional VALUES.

Re-running is safe: it recreates `$OUTPUT_DIR` from scratch.

Rough rules (regex + a small string parser):

- Skip tables whose rows hold *live user state* in Garlemald
  (`users`, `sessions`, `reserved_names`, and all `characters_*`).
  We keep the data tables.
- Strip MySQL pragmas (`/*!4010X ... */;`), `LOCK TABLES`/`UNLOCK TABLES`,
  `ALTER TABLE ... DISABLE/ENABLE KEYS`, `set autocommit`, `SET ...`,
  `DROP TABLE` (we use IF NOT EXISTS instead).
- Map MySQL types to SQLite affinities
  (`int(10) unsigned` -> `INTEGER`, `varchar(N)` -> `TEXT`, etc.).
- Drop secondary `KEY`/`UNIQUE KEY` lines inside `CREATE TABLE`
  (SQLite uses `CREATE INDEX` separately; we don't bother — the game
  server doesn't require them for correctness).
- Rewrite every ``INSERT INTO `tbl` VALUES (...)`` into
  `INSERT OR IGNORE INTO "tbl" (col1, ...) VALUES (...)`, using the
  column list parsed from the preceding `CREATE TABLE`.
- Convert the MySQL string-literal escape `\\'` to SQLite's doubled
  quote `''`, and drop backslash escapes of other characters.

Usage:
    scripts/convert-meteor-sql.py
    scripts/convert-meteor-sql.py --input ... --output ...
    scripts/convert-meteor-sql.py --dry-run
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import sys
from pathlib import Path

# Tables that shadow live user state in Garlemald's runtime DB. The Meteor
# dumps ship empty versions of most of these, but skipping them defensively
# keeps the migrations from ever stomping on live rows.
SKIP_TABLES = {
    'users',
    'sessions',
    'reserved_names',
    'characters',
    'characters_achievements',
    'characters_appearance',
    'characters_blacklist',
    'characters_chocobo',
    'characters_class_exp',
    'characters_class_levels',
    'characters_customattributes',
    'characters_friendlist',
    'characters_hotbar',
    'characters_inventory',
    'characters_inventory_equipment',
    'characters_linkshells',
    'characters_npclinkshell',
    'characters_parametersave',
    'characters_quest_completed',
    'characters_quest_guildleve_local',
    'characters_quest_guildleve_regional',
    'characters_quest_guildlevehistory',
    'characters_quest_scenario',
    'characters_retainers',
    'characters_statuseffect',
    'characters_timers',
}

# Ordered list: longer patterns first so `smallint` isn't partially matched
# by a shorter `int` rule.
TYPE_RULES: list[tuple[re.Pattern[str], str]] = [
    (re.compile(r'\btinyint\s*\(\s*\d+\s*\)(\s+unsigned)?', re.I), 'INTEGER'),
    (re.compile(r'\bsmallint\s*\(\s*\d+\s*\)(\s+unsigned)?', re.I), 'INTEGER'),
    (re.compile(r'\bmediumint\s*\(\s*\d+\s*\)(\s+unsigned)?', re.I), 'INTEGER'),
    (re.compile(r'\bint\s*\(\s*\d+\s*\)(\s+unsigned)?', re.I), 'INTEGER'),
    (re.compile(r'\bbigint\s*\(\s*\d+\s*\)(\s+unsigned)?', re.I), 'INTEGER'),
    (re.compile(r'\btinyint(\s+unsigned)?\b', re.I), 'INTEGER'),
    (re.compile(r'\bsmallint(\s+unsigned)?\b', re.I), 'INTEGER'),
    (re.compile(r'\bmediumint(\s+unsigned)?\b', re.I), 'INTEGER'),
    (re.compile(r'\bint(\s+unsigned)?\b', re.I), 'INTEGER'),
    (re.compile(r'\bbigint(\s+unsigned)?\b', re.I), 'INTEGER'),
    (re.compile(r'\bfloat(\s*\([^)]+\))?', re.I), 'REAL'),
    (re.compile(r'\bdouble(\s*\([^)]+\))?', re.I), 'REAL'),
    (re.compile(r'\bdecimal\s*\([^)]+\)', re.I), 'REAL'),
    (re.compile(r'\bvarchar\s*\(\s*\d+\s*\)(\s+CHARACTER SET \w+)?', re.I), 'TEXT'),
    (re.compile(r'\bchar\s*\(\s*\d+\s*\)', re.I), 'TEXT'),
    (re.compile(r'\blongtext\b', re.I), 'TEXT'),
    (re.compile(r'\btext\b', re.I), 'TEXT'),
    (re.compile(r'\bdatetime\b', re.I), 'TEXT'),
    (re.compile(r'\btimestamp\b', re.I), 'TEXT'),
    (re.compile(r'\blongblob\b', re.I), 'BLOB'),
    (re.compile(r'\bmediumblob\b', re.I), 'BLOB'),
    (re.compile(r'\bblob\b', re.I), 'BLOB'),
    (re.compile(r'\bbinary\s*\(\s*\d+\s*\)', re.I), 'BLOB'),
]

# Lines to drop outright from a CREATE TABLE body. Cheaper than a real
# parser; each is a full-line substring match against the trimmed line.
CREATE_DROP_PATTERNS = [
    re.compile(r'^\s*UNIQUE\s+KEY\s+', re.I),
    re.compile(r'^\s*PRIMARY\s+KEY\s*\(', re.I),  # handled separately below
    re.compile(r'^\s*KEY\s+', re.I),
    re.compile(r'^\s*INDEX\s+', re.I),
    re.compile(r'^\s*FULLTEXT\s+', re.I),
    re.compile(r'^\s*SPATIAL\s+', re.I),
    re.compile(r'^\s*CONSTRAINT\s+', re.I),
    re.compile(r'^\s*FOREIGN\s+KEY\s+', re.I),
]


def convert_string_escapes(literal: str) -> str:
    """Take a MySQL single-quoted string *including surrounding quotes* and
    return its SQLite equivalent. MySQL accepts backslash escapes
    (`\\'`, `\\\\`, `\\n`, etc.) that SQLite does not — SQLite only
    understands `''` as an embedded single quote."""
    assert literal.startswith("'") and literal.endswith("'"), literal
    body = literal[1:-1]
    out = []
    i = 0
    while i < len(body):
        ch = body[i]
        if ch == '\\' and i + 1 < len(body):
            nxt = body[i + 1]
            if nxt == "'":
                out.append("''")
            elif nxt == '\\':
                out.append('\\')
            elif nxt == '"':
                out.append('"')
            elif nxt == 'n':
                out.append('\n')
            elif nxt == 'r':
                out.append('\r')
            elif nxt == 't':
                out.append('\t')
            elif nxt == '0':
                out.append('\x00')
            else:
                # MySQL treats \x as just x for most other chars.
                out.append(nxt)
            i += 2
            continue
        if ch == "'":
            # A bare single quote in data should already be doubled in MySQL
            # output; still, normalise to SQLite's doubled form.
            out.append("''")
            i += 1
            continue
        out.append(ch)
        i += 1
    return "'" + ''.join(out) + "'"


_TOKEN_RE = re.compile(
    r"""
    (?P<string>'(?:[^'\\]|\\.|'')*')     # MySQL single-quoted string
    | (?P<ident>`[^`]+`)                 # backtick identifier
    | (?P<num>-?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?)  # number
    | (?P<null>\bNULL\b)
    | (?P<kw>\b[A-Za-z_][A-Za-z0-9_]*\b) # bareword / keyword
    | (?P<punct>[(),;])
    | (?P<ws>\s+)
    """,
    re.VERBOSE | re.DOTALL,
)


def tokenize(sql: str):
    """Yield (kind, text) tuples for a MySQL INSERT tail."""
    pos = 0
    while pos < len(sql):
        m = _TOKEN_RE.match(sql, pos)
        if not m:
            # Single unknown char — emit as-is to keep the stream alive.
            yield 'raw', sql[pos]
            pos += 1
            continue
        kind = m.lastgroup
        text = m.group()
        pos = m.end()
        if kind == 'ws':
            continue
        yield kind, text


def parse_create_table(text: str) -> tuple[str, list[tuple[str, str]]] | None:
    """Return (table_name, [(col_name, col_def_sqlite), ...]) from a CREATE
    TABLE statement, or None if the text isn't a CREATE TABLE at all.
    `col_def_sqlite` has MySQL types mapped and width/default specifiers
    preserved verbatim."""
    m = re.search(
        r'CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?`([^`]+)`\s*\((.*?)\)\s*ENGINE\s*=',
        text, re.I | re.DOTALL,
    )
    if not m:
        return None
    table = m.group(1)
    body = m.group(2)

    columns: list[tuple[str, str]] = []
    # Split top-level commas only (column definitions never nest parens
    # deeper than 1 in these dumps, e.g. `int(10) unsigned`).
    depth = 0
    start = 0
    parts: list[str] = []
    for i, ch in enumerate(body):
        if ch == '(':
            depth += 1
        elif ch == ')':
            depth -= 1
        elif ch == ',' and depth == 0:
            parts.append(body[start:i])
            start = i + 1
    parts.append(body[start:])

    for raw in parts:
        line = raw.strip()
        if not line:
            continue
        if any(p.match(line) for p in CREATE_DROP_PATTERNS):
            continue
        m2 = re.match(r'`([^`]+)`\s+(.+)$', line, re.DOTALL)
        if not m2:
            continue
        col_name = m2.group(1)
        col_def = m2.group(2).strip().rstrip(',')
        # SQLite doesn't know AUTO_INCREMENT; convert to AUTOINCREMENT and
        # (if this looks like the PK column) strip the `unsigned NOT NULL`
        # plumbing — the PK is declared below.
        col_def = re.sub(r'\bAUTO_INCREMENT\b', 'AUTOINCREMENT', col_def, flags=re.I)
        for pat, repl in TYPE_RULES:
            col_def = pat.sub(repl, col_def)
        # Collapse double-spaces and trailing keyword noise.
        col_def = re.sub(r'\s+', ' ', col_def).strip()
        columns.append((col_name, col_def))

    return table, columns


def build_create_table_sqlite(table: str, columns: list[tuple[str, str]],
                              pk_cols: list[str]) -> str:
    # SQLite only accepts `AUTOINCREMENT` when it is attached to a single
    # `INTEGER PRIMARY KEY AUTOINCREMENT` column. If Meteor's MySQL column
    # uses AUTO_INCREMENT *and* the table's PRIMARY KEY targets that one
    # column, promote the column definition and drop the separate PK
    # clause. Otherwise, strip the AUTOINCREMENT keyword so SQLite parses.
    has_autoinc = [i for i, (_, d) in enumerate(columns)
                   if re.search(r'\bAUTOINCREMENT\b', d)]
    promoted_col: str | None = None
    col_lines: list[str] = []
    for name, defn in columns:
        if (len(has_autoinc) == 1
                and columns[has_autoinc[0]][0] == name
                and len(pk_cols) == 1
                and pk_cols[0] == name):
            # Promote: "col" INTEGER PRIMARY KEY AUTOINCREMENT (drop
            # NOT NULL / DEFAULT — PK on INTEGER is implicitly NOT NULL).
            col_lines.append(f'    "{name}" INTEGER PRIMARY KEY AUTOINCREMENT')
            promoted_col = name
            continue
        # No promotion available: SQLite can't honour AUTOINCREMENT on a
        # non-PK column, so strip it.
        defn = re.sub(r'\s*\bAUTOINCREMENT\b', '', defn)
        col_lines.append(f'    "{name}" {defn}')
    if pk_cols and promoted_col is None:
        pk = ', '.join(f'"{c}"' for c in pk_cols)
        col_lines.append(f'    PRIMARY KEY ({pk})')
    return (
        f'CREATE TABLE IF NOT EXISTS "{table}" (\n'
        + ',\n'.join(col_lines)
        + '\n);\n'
    )


def extract_primary_key(create_text: str) -> list[str]:
    m = re.search(r'PRIMARY\s+KEY\s*\(([^)]+)\)', create_text, re.I)
    if not m:
        return []
    inner = m.group(1)
    return [c.strip().strip('`"') for c in inner.split(',')]


def rewrite_insert(insert_sql: str, fallback_columns: list[str]) -> list[str]:
    """Return one or more SQLite `INSERT OR IGNORE INTO "tbl" (cols) VALUES
    (...);` statements. The input is one MySQL `INSERT` statement (which
    may be a multi-row form or a single-row form)."""
    m = re.match(
        r'INSERT\s+INTO\s+`([^`]+)`\s*(\([^)]+\))?\s*VALUES\s*(.+);\s*$',
        insert_sql, re.I | re.DOTALL,
    )
    if not m:
        return []
    table = m.group(1)
    col_clause = m.group(2)
    values_tail = m.group(3).strip()
    if col_clause:
        cols = [c.strip().strip('`"') for c in col_clause[1:-1].split(',')]
    else:
        cols = fallback_columns
    if not cols:
        raise RuntimeError(f"no column list for INSERT into {table}")

    # Split values_tail into row tuples. Need a real tokenizer here because
    # rows can contain strings with commas, parens, and backslash-escapes.
    rows: list[str] = []
    pos = 0
    depth = 0
    start = None
    i = 0
    raw = values_tail
    while i < len(raw):
        ch = raw[i]
        if ch == "'":
            # Skip a MySQL string literal, respecting \' and ''.
            i += 1
            while i < len(raw):
                c = raw[i]
                if c == '\\' and i + 1 < len(raw):
                    i += 2
                    continue
                if c == "'":
                    if i + 1 < len(raw) and raw[i + 1] == "'":
                        i += 2
                        continue
                    i += 1
                    break
                i += 1
            continue
        if ch == '(':
            if depth == 0:
                start = i
            depth += 1
            i += 1
            continue
        if ch == ')':
            depth -= 1
            if depth == 0 and start is not None:
                rows.append(raw[start:i + 1])
                start = None
            i += 1
            continue
        i += 1

    if not rows:
        return []

    # Now rebuild each row with escape-normalised strings.
    rebuilt_rows: list[str] = []
    for row in rows:
        # row is "(v1, v2, ...)"; re-tokenise.
        assert row.startswith('(') and row.endswith(')'), row
        inner = row[1:-1]
        values: list[str] = []
        j = 0
        cur_start = 0
        cdepth = 0
        while j < len(inner):
            c = inner[j]
            if c == "'":
                j += 1
                while j < len(inner):
                    cc = inner[j]
                    if cc == '\\' and j + 1 < len(inner):
                        j += 2
                        continue
                    if cc == "'":
                        j += 1
                        break
                    j += 1
                continue
            if c == '(':
                cdepth += 1
                j += 1
                continue
            if c == ')':
                cdepth -= 1
                j += 1
                continue
            if c == ',' and cdepth == 0:
                values.append(inner[cur_start:j].strip())
                cur_start = j + 1
                j += 1
                continue
            j += 1
        values.append(inner[cur_start:].strip())

        normalised: list[str] = []
        for v in values:
            if v.startswith("'") and v.endswith("'"):
                normalised.append(convert_string_escapes(v))
            else:
                normalised.append(v)
        rebuilt_rows.append('(' + ', '.join(normalised) + ')')

    col_list = ', '.join(f'"{c}"' for c in cols)
    stmts: list[str] = []
    # Batch rows into statements of at most ~500 rows to keep individual
    # statements small and parseable without blowing SQLite's default SQL
    # length cap on slow machines.
    BATCH = 500
    for start in range(0, len(rebuilt_rows), BATCH):
        chunk = rebuilt_rows[start:start + BATCH]
        stmts.append(
            f'INSERT OR IGNORE INTO "{table}" ({col_list}) VALUES\n    '
            + ',\n    '.join(chunk)
            + ';'
        )
    return stmts


def split_statements(sql: str) -> list[str]:
    """Split a MySQL file into individual statements on `;` at depth 0,
    respecting string literals. Drops `/*! ... */` pragma comments and `--`
    line comments."""
    # Remove /*! ... */ block pragmas in one pass (they never contain ;
    # followed by real SQL).
    sql = re.sub(r'/\*!\d+\s[^*]*(?:\*(?!/)[^*]*)*\*/;?', '', sql, flags=re.DOTALL)
    # Remove -- line comments.
    sql = re.sub(r'(?m)^\s*--.*$', '', sql)

    out: list[str] = []
    i = 0
    start = 0
    while i < len(sql):
        ch = sql[i]
        if ch == "'":
            i += 1
            while i < len(sql):
                c = sql[i]
                if c == '\\' and i + 1 < len(sql):
                    i += 2
                    continue
                if c == "'":
                    i += 1
                    break
                i += 1
            continue
        if ch == '`':
            i += 1
            while i < len(sql) and sql[i] != '`':
                i += 1
            if i < len(sql):
                i += 1
            continue
        if ch == ';':
            stmt = sql[start:i + 1].strip()
            if stmt and stmt != ';':
                out.append(stmt)
            start = i + 1
            i += 1
            continue
        i += 1
    tail = sql[start:].strip()
    if tail:
        out.append(tail)
    return out


IGNORE_STATEMENT_PREFIXES = (
    'DROP TABLE',
    'SET ',
    'LOCK TABLES',
    'UNLOCK TABLES',
    'ALTER TABLE',
    'set autocommit',
    'SET @',
    'commit',
    'COMMIT',
    'begin',
    'BEGIN',
)


def is_ignored(stmt: str) -> bool:
    s = stmt.lstrip()
    return any(s.upper().startswith(p.upper()) for p in IGNORE_STATEMENT_PREFIXES)


def convert_file(src: Path) -> tuple[str, str] | None:
    """Convert a single .sql file. Returns (table_name, output_sql) or None
    if the file should be skipped."""
    text = src.read_text(encoding='utf-8', errors='replace')
    statements = split_statements(text)

    create: tuple[str, list[tuple[str, str]]] | None = None
    inserts: list[str] = []
    pk_cols: list[str] = []
    for stmt in statements:
        upper = stmt.upper().lstrip()
        if upper.startswith('CREATE TABLE'):
            create = parse_create_table(stmt)
            pk_cols = extract_primary_key(stmt)
            continue
        if upper.startswith('INSERT'):
            if create is None:
                raise RuntimeError(f"INSERT before CREATE TABLE in {src}")
            cols = [c for c, _ in create[1]]
            inserts.extend(rewrite_insert(stmt, cols))
            continue
        if is_ignored(stmt):
            continue
        # Anything else silently dropped — the MySQL dumps don't use
        # triggers, views, or stored procs.

    if create is None:
        return None
    table = create[0]
    if table in SKIP_TABLES:
        return None

    lines: list[str] = []
    lines.append(f'-- Ported from project-meteor-mirror/Data/sql/{src.name}')
    lines.append(f'-- Table: {table}')
    lines.append('')
    lines.append(build_create_table_sqlite(table, create[1], pk_cols))
    lines.extend(inserts)
    output = '\n'.join(lines).rstrip() + '\n'
    return table, output


def main() -> int:
    parser = argparse.ArgumentParser()
    here = Path(__file__).resolve().parent
    default_input = here.parent.parent / 'project-meteor-mirror' / 'Data' / 'sql'
    default_output = here.parent / 'common' / 'sql' / 'seed'
    parser.add_argument('--input', type=Path, default=default_input)
    parser.add_argument('--output', type=Path, default=default_output)
    parser.add_argument('--dry-run', action='store_true')
    args = parser.parse_args()

    if not args.input.is_dir():
        print(f"input dir not found: {args.input}", file=sys.stderr)
        return 2

    files = sorted(p for p in args.input.glob('*.sql'))
    if not files:
        print(f"no .sql files under {args.input}", file=sys.stderr)
        return 2

    converted: list[tuple[str, str, str]] = []  # (basename, table, output)
    for f in files:
        try:
            result = convert_file(f)
        except Exception as e:
            print(f"FAILED {f.name}: {e}", file=sys.stderr)
            raise
        if result is None:
            print(f"skip   {f.name}")
            continue
        table, output = result
        print(f"ok     {f.name} -> table '{table}' ({len(output):>8} bytes)")
        converted.append((f.stem, table, output))

    if args.dry_run:
        return 0

    # Recreate output dir from scratch so removed/renamed source files don't
    # leave stale migrations behind.
    if args.output.exists():
        shutil.rmtree(args.output)
    args.output.mkdir(parents=True)

    for idx, (stem, _table, output) in enumerate(converted, 1):
        out_path = args.output / f'{idx:03d}_{stem}.sql'
        out_path.write_text(output, encoding='utf-8')

    print(f"\nwrote {len(converted)} migrations to {args.output}")
    return 0


if __name__ == '__main__':
    sys.exit(main())
