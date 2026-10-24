#!/usr/bin/env python3
"""Generate open-world monster spawns for La Noscea, Coerthas, the Black Shroud and Thanalan.

Writes `common/sql/seed/101_world_monster_spawns.sql`: battle-NPC pools, groups and spawn
locations (`spawnType = 0`, so the map server spawns them at boot), plus classPath repairs for
the monster actor classes they use.

Sources (see `data/README.md`):
  * exact retail positions decoded from 1.23b packet captures (`xivl_spawn_observations.csv`);
  * Mooglebox's 2012 regional camp maps (`xivl_mooglebox_markers.csv`) - name, level, count, map
    coordinates;
  * Gamer Escape 1.x monster pages (`xivl_ge_mob_locations.csv`) - zone + map coordinates, and
    the name -> client actor class join (`xivl_ge_mob_client_crosscheck.csv`).

Map coordinates convert to world x/z at 100 world units per map unit with a per-region offset,
fitted against NPCs and aetherytes whose positions the server already has (median error about
20 units). Ground height comes from the nearest known positions in the same zone (captures,
NPCs, aetherytes, gathering points and logged player positions); the client does the rest.

Usage: generate.py [--db data/garlemald.db] [--client "<1.23b install>"]
"""

import argparse
import collections
import csv
import json
import math
import random
import re
import sqlite3
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
DATA = HERE / "data"
OUT = REPO / "common" / "sql" / "seed" / "101_world_monster_spawns.sql"

# Region -> (x offset, z offset); world = 100 * map + offset.
REGIONS = {
    101: ("La Noscea", -2473, -2945),
    102: ("Coerthas", -3263, -1259),
    103: ("Black Shroud", -3043, -3752),
    104: ("Thanalan", -2640, -3044),
}
REGION_BY_NAME = {v[0]: k for k, v in REGIONS.items()}
FIELD_ZONES = {
    101: [128, 129, 130, 135],
    102: [143, 144, 145, 147, 148],
    103: [150, 151, 152, 153, 154],
    104: [170, 171, 172, 173, 174],
}
ZONE_BY_TAG = {
    "sea0Fld01": 128, "sea0Fld02": 129, "sea0Fld03": 130, "sea0Fld04": 135,
    "roc0Fld01": 143, "roc0Fld02": 144, "roc0Fld03": 145, "roc0Fld04": 147, "roc0Fld05": 148,
    "fst0Fld01": 150, "fst0Fld02": 151, "fst0Fld03": 152, "fst0Fld04": 153, "fst0Fld05": 154,
    "wil0Fld01": 170, "wil0Fld02": 171, "wil0Fld03": 172, "wil0Fld04": 173, "wil0Fld05": 174,
}

# Id ranges owned by this seed (cleared and rewritten on each run).
POOL_BASE, GROUP_BASE, BNPC_BASE = 10000, 10000, 100000
DEFAULT_AMOUNT, MAX_AMOUNT = 8, 16
GE_CAMP_AMOUNT = 6
RESPAWN_SECONDS = 90

# Client race name (or family) -> genus name in server_battlenpc_genus.
GENUS_ALIASES = {
    "aldgoats": "Aldgoat", "wolf/hyena/hellhound": "Wolf", "opo-opo": "Opo-opo", "couerl": "Coeurl",
    "rat": "Rodent", "hedgemole": "Mole", "sabotender": "Cactuar", "flytrap": "Flying-trap",
    "jellyfish": "Jelly-fish", "crab": "Megalo-crab", "amlaj'aa": "Amaalja", "ixal": "Ixal",
    "antling": "Ant-ring", "weevil": "Beetle", "zombie": "The-Damned", "firefly": "Will-O-Wisp",
    "cockatrice": "Dodo", "angler": "Orobon", "treant": "Morbol", "earth elemental": "Earth-Elemental",
    "fire elemental": "Fire-Elemental", "ice elemental": "Ice-Elemental",
    "water elemental": "Water-Elemental", "lightning elemental": "Lightning-Elemental",
    "air elemental": "Wind-Elemental",
}


def parse_points(text):
    """'9-21, 14-33' / '(45,51)' / '53.25,32' -> [(x, y), ...] in map units."""
    pts = []
    for m in re.finditer(r"(\d+(?:\.\d+)?)\s*[-,]\s*(\d+(?:\.\d+)?)", text or ""):
        x, y = float(m[1]), float(m[2])
        if 0 < x < 70 and 0 < y < 70:
            pts.append((x, y))
    return pts


def to_world(region, mx, my):
    _, bx, bz = REGIONS[region]
    return 100.0 * mx + bx, 100.0 * my + bz


def hp_curve():
    """Level -> max HP from retail captures (median per level, log-linear between levels)."""
    by = collections.defaultdict(list)
    with open(DATA / "xivl_mob_level_hp.csv") as f:
        for r in csv.DictReader(f):
            by[int(r["level"])].append(int(r["hpMax"]))
    pts = sorted((lv, sorted(v)[len(v) // 2]) for lv, v in by.items())

    def hp(level):
        if level <= pts[0][0]:
            return pts[0][1]
        for (l0, h0), (l1, h1) in zip(pts, pts[1:]):
            if l0 <= level <= l1:
                t = (level - l0) / (l1 - l0)
                return round(math.exp(math.log(h0) + t * (math.log(h1) - math.log(h0))))
        (l0, h0), (l1, h1) = pts[-2], pts[-1]
        return round(h1 * (h1 / h0) ** ((level - l1) / (l1 - l0)))

    return hp


HP_FOR_LEVEL = None


def level_range(text, lo=None, hi=None):
    nums = [int(n) for n in re.findall(r"\d+", text or "")]
    if nums:
        return min(nums), max(nums)
    if lo:
        return lo, hi or lo
    return None


class World:
    def __init__(self, db):
        self.db = db
        self.zone_region = {}
        self.zone_names = {}
        for zid, reg, name in db.execute("SELECT id, regionId, placeName FROM server_zones"):
            self.zone_region[zid] = reg
            self.zone_names[zid] = name
        self.samples = collections.defaultdict(list)  # zone -> [(x, y, z)]

    def add_sample(self, zone, x, y, z):
        if self.zone_region.get(zone) in REGIONS and zone in FIELD_ZONES[self.zone_region[zone]]:
            self.samples[zone].append((x, y, z))

    def load_samples(self):
        db = self.db
        for zone, x, y, z in db.execute(
            "SELECT zoneId, positionX, positionY, positionZ FROM server_spawn_locations"
        ):
            self.add_sample(zone, x, y, z)
        for zone, x, y, z in db.execute(
            "SELECT zoneId, positionX, positionY, positionZ FROM server_gather_node_spawns"
        ):
            self.add_sample(zone, x, y, z)
        with open(DATA / "height_samples_player.csv") as f:
            for r in csv.DictReader(f):
                self.add_sample(int(r["zoneId"]), float(r["x"]), float(r["y"]), float(r["z"]))
        with open(DATA / "xivl_spawn_observations.csv") as f:
            for r in csv.DictReader(f):
                zone = ZONE_BY_TAG.get(r["zoneTag"].rstrip("a"))
                if zone:
                    self.add_sample(zone, float(r["x"]), float(r["y"]), float(r["z"]))

    def nearest(self, zones, x, z, k=1):
        best = []
        for zone in zones:
            for sx, sy, sz in self.samples.get(zone, ()):
                best.append(((sx - x) ** 2 + (sz - z) ** 2, zone, sy))
        best.sort()
        return best[:k]

    def zone_for(self, region, x, z, hint=None):
        """The field zone a world point belongs to: the GE-named zone when given, else the zone
        of the nearest known positions (a 7-nearest vote)."""
        if hint:
            return hint
        votes = collections.Counter(zone for _, zone, _ in self.nearest(FIELD_ZONES[region], x, z, 7))
        return votes.most_common(1)[0][0] if votes else FIELD_ZONES[region][0]

    def height(self, zone, x, z):
        """Inverse-distance blend of the four nearest heights in the zone."""
        near = self.nearest([zone], x, z, 4)
        if not near:
            near = self.nearest(FIELD_ZONES[self.zone_region[zone]], x, z, 4)
        if not near:
            return 0.0, 1e9
        if near[0][0] < 1.0:
            return near[0][2], 0.0
        w = [1.0 / d for d, _, _ in near]
        y = sum(wi * yi for wi, (_, _, yi) in zip(w, near)) / sum(w)
        return y, math.sqrt(near[0][0])


class Classes:
    """Monster name -> usable client actor class (has an appearance row and a valid classPath)."""

    def __init__(self, db, client_dir):
        self.path = {i: p for i, p in db.execute(
            "SELECT id, classPath FROM gamedata_actor_class WHERE id BETWEEN 2000000 AND 2999999")}
        self.has_look = {i for (i,) in db.execute(
            "SELECT id FROM gamedata_actor_appearance WHERE id BETWEEN 2000000 AND 2999999")}
        self.genus = {n.lower(): g for g, n in db.execute("SELECT genusId, name FROM server_battlenpc_genus")}
        self.scripts = self.client_scripts(client_dir)
        self.rows = {}
        with open(DATA / "xivl_ge_mob_client_crosscheck.csv") as f:
            for r in csv.DictReader(f):
                self.rows[r["mob_name"].lower()] = r
        # Race -> monster script folder, from classes whose classPath is already known.
        votes = collections.defaultdict(collections.Counter)
        self.folder_class = collections.defaultdict(collections.Counter)
        for r in self.rows.values():
            for i in map(int, r["mob_range_actor_ids"].split()):
                p = self.path.get(i) or ""
                if p.startswith("/Chara/Npc/Monster/"):
                    votes[r["client_race_name"]][p.split("/")[4]] += 1
        for p in self.path.values():
            if p and p.startswith("/Chara/Npc/Monster/"):
                self.folder_class[p.split("/")[4]][p] += 1
        self.race_folder = {race: c.most_common(1)[0][0] for race, c in votes.items()}
        self.repairs = {}

    @staticmethod
    def client_scripts(client_dir):
        """Decoded monster script paths from the install's client/script file names (the names
        are a letter substitution: a-j <-> 9-0, k-z reversed)."""
        if not client_dir:
            return set()

        def dec(ch):
            if ch.isdigit():
                return chr(ord("a") + 9 - int(ch))
            if "k" <= ch <= "z":
                return chr(ord("k") + ord("z") - ord(ch))
            return ch

        root = Path(client_dir) / "client" / "script"
        out = set()
        for f in root.rglob("*.lpb"):
            rel = f.relative_to(root).as_posix().replace(".le.lpb", "")
            name = "".join(dec(c) for c in rel)
            if name.startswith("chara/npc/monster/"):
                out.add(name)
        return out

    def folder_for(self, race, existing_folders):
        if race in self.race_folder:
            return self.race_folder[race]
        key = re.sub(r"[^a-z]", "", race.lower().split("/")[0])
        for folder in existing_folders:
            if folder.lower() == key or folder.lower().rstrip("s") == key.rstrip("s"):
                return folder
        for s in self.scripts:
            folder = s.split("/")[3]
            if folder == key or folder.rstrip("s") == key.rstrip("s"):
                return folder.capitalize()
        return None

    def class_path_for(self, race):
        folder = self.folder_for(race, self.folder_class.keys())
        if not folder:
            return None
        known = self.folder_class.get(folder)
        if known:
            return known.most_common(1)[0][0]
        lower = folder.lower()
        if f"chara/npc/monster/{lower}/{lower}standard" in self.scripts:
            return f"/Chara/Npc/Monster/{folder}/{folder}Standard"
        return None

    def pick(self, name, want_path=None):
        """(actor class id, row) for a monster name, or None."""
        r = self.rows.get(name.lower())
        if not r:
            return None
        ids = [int(i) for i in r["mob_range_actor_ids"].split() if int(i) in self.has_look]
        if not ids:
            return None
        if want_path:
            for i in ids:
                if (self.path.get(i) or "").lower().endswith("/" + want_path.lower()):
                    return i, r
        for i in ids:
            if self.path.get(i):
                return i, r
        repaired = self.class_path_for(r["client_race_name"])
        if not repaired:
            return None
        self.repairs[ids[0]] = repaired
        self.path[ids[0]] = repaired
        return ids[0], r

    def genus_for(self, r):
        for key in (r["client_race_name"], r["ge_family"]):
            k = (key or "").lower()
            if k in GENUS_ALIASES:
                return self.genus[GENUS_ALIASES[k].lower()]
            if k in self.genus:
                return self.genus[k]
            if k.rstrip("s") in self.genus:
                return self.genus[k.rstrip("s")]
        return self.genus["wolf"]


class Builder:
    def __init__(self, world, classes):
        self.world, self.classes = world, classes
        self.pools = {}  # (class id, aggro) -> pool id
        self.groups = []  # dicts
        self.spawns = []  # (bnpc, group, x, y, z, rot)
        self.skipped = collections.Counter()
        self.far_heights = 0

    def pool(self, cid, row, aggressive):
        key = (cid, aggressive)
        if key not in self.pools:
            self.pools[key] = (POOL_BASE + len(self.pools), row["mob_name"], self.classes.genus_for(row))
        return self.pools[key][0]

    def group(self, cid, row, zone, levels, aggressive, source):
        lo, hi = levels
        # Gamer Escape's HP column mixes level brackets (a level-1 marmot listed at 720);
        # the retail capture curve gives 80 at level 1.
        hp = HP_FOR_LEVEL(lo)
        gid = GROUP_BASE + len(self.groups)
        self.groups.append(dict(id=gid, pool=self.pool(cid, row, aggressive), name=row["mob_name"],
                                lo=lo, hi=hi, hp=hp, zone=zone, source=source))
        return gid

    def place(self, gid, zone, x, z, rot, y=None):
        if y is None:
            y, dist = self.world.height(zone, x, z)
            if dist > 250:
                self.far_heights += 1
        self.spawns.append((BNPC_BASE + len(self.spawns), gid, x, y, z, rot))

    def camp(self, gid, zone, cx, cz, amount, seed):
        rng = random.Random(seed)
        radius = 10 + 3 * amount
        for i in range(amount):
            a = 2 * math.pi * (i + rng.random() * 0.6) / amount
            d = radius * (0.35 + 0.65 * rng.random())
            self.place(gid, zone, cx + d * math.cos(a), cz + d * math.sin(a), rng.uniform(-math.pi, math.pi))

    def levels_for(self, row, text=""):
        lv = level_range(text) or level_range("", int(row["level_low"]) if row["level_low"].isdigit() else None,
                                              int(row["level_high"]) if row["level_high"].isdigit() else None)
        return lv or (1, 1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--db", default=str(REPO / "data" / "garlemald.db"))
    ap.add_argument("--client", default="")
    args = ap.parse_args()
    global HP_FOR_LEVEL
    HP_FOR_LEVEL = hp_curve()
    db = sqlite3.connect(f"file:{args.db}?mode=ro", uri=True)
    world = World(db)
    world.load_samples()
    classes = Classes(db, args.client)
    b = Builder(world, classes)
    zone_id_by_name = collections.defaultdict(list)
    for zid, name in world.zone_names.items():
        if world.zone_region.get(zid) in REGIONS and zid in FIELD_ZONES[world.zone_region[zid]]:
            zone_id_by_name[name].append(zid)

    # 1. Exact retail positions.
    exact = collections.defaultdict(list)  # name -> [(x, z)]
    with open(DATA / "xivl_spawn_observations.csv") as f:
        obs = [r for r in csv.DictReader(f) if "/Monster/" in r["classPath"]]
    ge_rows = list(csv.DictReader(open(DATA / "xivl_ge_mob_locations.csv")))
    ge_names_by_zone = collections.defaultdict(set)
    for r in ge_rows:
        ge_names_by_zone[r["zone"]].add(r["mob_name"])
    by_path = collections.defaultdict(list)
    for name, r in classes.rows.items():
        for i in map(int, r["mob_range_actor_ids"].split()):
            p = (classes.path.get(i) or "").split("/")[-1].lower()
            if p:
                by_path[p].append(r["mob_name"])
    aggressive_names = {r["mob_name"] for r in ge_rows if "A" in r["behavior_flags"]}
    groups_exact = collections.defaultdict(list)
    for r in obs:
        zone = ZONE_BY_TAG.get(r["zoneTag"].rstrip("a"))
        if zone:
            groups_exact[(zone, r["baseClass"])].append(r)
    for (zone, base), recs in sorted(groups_exact.items()):
        zname = world.zone_names[zone]
        names = by_path.get(base.lower(), [])
        names = sorted(names, key=lambda n: (n not in ge_names_by_zone[zname], n))
        picked = None
        for n in names:
            picked = classes.pick(n, base)
            if picked:
                break
        if not picked:
            b.skipped[f"exact:{base}"] += len(recs)
            continue
        cid, row = picked
        ge_lv = next((g["level_range"] for g in ge_rows if g["mob_name"] == row["mob_name"]
                      and g["zone"] == zname and g["level_range"]), "")
        gid = b.group(cid, row, zone, b.levels_for(row, ge_lv),
                      row["mob_name"] in aggressive_names, "capture")
        for r in recs:
            b.place(gid, zone, float(r["x"]), float(r["z"]), float(r["rotation"]), float(r["y"]))
            exact[row["mob_name"]].append((float(r["x"]), float(r["z"])))

    def near_exact(name, x, z, dist=250):
        return any((ex - x) ** 2 + (ez - z) ** 2 < dist * dist for ex, ez in exact.get(name, ()))

    # 2. Mooglebox camp markers.
    camps = collections.defaultdict(list)  # name -> [(x, z)]
    with open(DATA / "xivl_mooglebox_markers.csv") as f:
        markers = list(csv.DictReader(f))
    for m in markers:
        region = REGION_BY_NAME.get(m["region"])
        if not region or not m["coordinate_x"]:
            continue
        x, z = to_world(region, float(m["coordinate_x"]), float(m["coordinate_y"]))
        cands = [c.strip() for c in json.loads(m["parsed_candidates"] or "[]")] or [m["label_raw"]]
        # "Ixali Strongbeak/Fencer/Bravewing": later slots share the first label's leading words.
        head = cands[0].split()[:-1]
        cands = [cands[0]] + [c if len(c.split()) > 1 or not head else " ".join(head + [c]) for c in cands[1:]]
        amount = int(float(m["amount_max"] or m["amount_min"])) if m["amount_min"] else DEFAULT_AMOUNT
        amount = max(1, min(MAX_AMOUNT, amount))
        share = max(1, round(amount / len(cands)))
        for k, name in enumerate(cands):
            picked = classes.pick(name)
            if not picked:
                b.skipped[f"marker:{name}"] += 1
                continue
            cid, row = picked
            if near_exact(row["mob_name"], x, z):
                continue
            zone = world.zone_for(region, x, z)
            lv = level_range(m["level_text"]) or b.levels_for(row)
            gid = b.group(cid, row, zone, lv, m["aggression_marker"] == "1", "mooglebox")
            b.camp(gid, zone, x + 15 * k, z, share, f"{m['region']}:{m['marker_order']}:{k}")
            camps[row["mob_name"]].append((x, z))

    # 3. Gamer Escape coordinates not already covered by a camp or a capture.
    for g in ge_rows:
        zids = zone_id_by_name.get(g["zone"])
        if not zids:
            continue
        zone = zids[0]
        region = world.zone_region[zone]
        row = classes.rows.get(g["mob_name"].lower())
        if not row or "persistent" not in row["spawn_container"] or row["notorious_monster"] == "Y":
            continue
        for mx, my in parse_points(g["map_coords"]):
            x, z = to_world(region, mx, my)
            if near_exact(g["mob_name"], x, z) or any(
                    (cx - x) ** 2 + (cz - z) ** 2 < 200 ** 2 for cx, cz in camps.get(g["mob_name"], ())):
                continue
            picked = classes.pick(g["mob_name"])
            if not picked:
                b.skipped[f"ge:{g['mob_name']}"] += 1
                continue
            cid, row = picked
            gid = b.group(cid, row, zone, b.levels_for(row, g["level_range"]), "A" in g["behavior_flags"], "gamerescape")
            b.camp(gid, zone, x, z, GE_CAMP_AMOUNT, f"ge:{g['mob_name']}:{mx}:{my}")
            camps[g["mob_name"]].append((x, z))

    write_sql(b, classes)
    per_zone = collections.Counter()
    gz = {g["id"]: g["zone"] for g in b.groups}
    for s in b.spawns:
        per_zone[gz[s[1]]] += 1
    print(f"pools {len(b.pools)}  groups {len(b.groups)}  monsters {len(b.spawns)}  "
          f"classPath repairs {len(classes.repairs)}  heights >250 from a sample {b.far_heights}")
    for zone, n in sorted(per_zone.items()):
        print(f"  {zone} {world.zone_names[zone]}: {n}")
    print("skipped:", sum(b.skipped.values()), b.skipped.most_common(12))


def sql_str(s):
    return "'" + str(s).replace("'", "''") + "'"


def write_sql(b, classes):
    lines = [
        "-- Generated by tools/world_spawns/generate.py - do not edit by hand.",
        "-- Open-world monsters for La Noscea, Coerthas, the Black Shroud and Thanalan.",
        "-- Positions: retail 1.23b packet captures where available, otherwise Mooglebox (2012)",
        "-- camp maps and Gamer Escape 1.x monster pages, via the XIVLegacy xivl-captures",
        "-- research data (CC BY 4.0, https://github.com/XIVLegacy/xivl-captures).",
        "",
        "-- The test wharf rats (group 1) are not meant to spawn on their own.",
        "UPDATE server_battlenpc_groups SET spawnType = 1 WHERE groupId = 1;",
        "",
        f"DELETE FROM server_battlenpc_spawn_locations WHERE bnpcId >= {BNPC_BASE};",
        f"DELETE FROM server_battlenpc_groups WHERE groupId >= {GROUP_BASE};",
        f"DELETE FROM server_battlenpc_pools WHERE poolId >= {POOL_BASE};",
        "",
        "-- Monster classes the server had without a classPath (the client needs one).",
    ]
    for cid, path in sorted(classes.repairs.items()):
        lines.append(f"UPDATE gamedata_actor_class SET classPath = {sql_str(path)} "
                     f"WHERE id = {cid} AND (classPath IS NULL OR classPath = '');")
    lines.append("")
    for (cid, aggressive), (pid, name, genus) in sorted(b.pools.items(), key=lambda t: t[1][0]):
        lines.append(
            "INSERT INTO server_battlenpc_pools (poolId, actorClassId, name, genusId, currentJob, combatSkill, "
            "combatDelay, combatDmgMult, aggroType, immunity, linkType, spellListId, skillListId) VALUES "
            f"({pid}, {cid}, {sql_str(name)}, {genus}, 0, 1, 4200, 1.0, {1 if aggressive else 0}, 0, 0, 0, 0);")
    lines.append("")
    for g in b.groups:
        lines.append(
            "INSERT INTO server_battlenpc_groups (groupId, poolId, scriptName, minLevel, maxLevel, respawnTime, "
            "hp, mp, dropListId, allegiance, spawnType, animationId, actorState, privateAreaName, "
            "privateAreaLevel, zoneId) VALUES "
            f"({g['id']}, {g['pool']}, {sql_str(re.sub(r'[^a-z0-9]+', '_', g['name'].lower()).strip('_'))}, "
            f"{g['lo']}, {g['hi']}, {RESPAWN_SECONDS}, {g['hp']}, 0, 0, 0, 0, 0, 0, '', 0, {g['zone']});"
            f" -- {g['source']}")
    lines.append("")
    gname = {g["id"]: g["name"] for g in b.groups}
    for bnpc, gid, x, y, z, rot in b.spawns:
        lines.append(
            "INSERT INTO server_battlenpc_spawn_locations (bnpcId, customDisplayName, groupId, positionX, "
            f"positionY, positionZ, rotation) VALUES ({bnpc}, {sql_str(re.sub(r'[^a-z0-9]+', '_', gname[gid].lower()).strip('_'))}, "
            f"{gid}, {x:.3f}, {y:.3f}, {z:.3f}, {rot:.4f});")
    OUT.write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
