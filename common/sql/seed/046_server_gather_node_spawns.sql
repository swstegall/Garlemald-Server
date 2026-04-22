-- World placement of gathering nodes. Parallels `server_spawn_locations`
-- for BattleNPCs/ENPCs, but carries an extra pair of fields the
-- harvesting system needs:
--
--   harvestNodeId — FK into `gamedata_gather_nodes.id`. Tells the
--                   DummyCommand minigame which template (grade /
--                   attempts / item pool) this physical node exposes.
--   harvestType   — which harvest command opens the minigame for this
--                   node. Matches the command ids used by 1.x's Lua
--                   scripts: 22002 Quarry/Mine, 22003 Log, 22004 Fish.
--                   Botany would land here too once that is added.
--
-- For the first pass this table is seeded with a pair of tutorial-area
-- mining outcrops so the loader has something to round-trip against
-- in tests. Real zone placements (mozk-tabetai dump has 531 nodes
-- across the live maps) are a follow-on.

DROP TABLE IF EXISTS "server_gather_node_spawns";
CREATE TABLE IF NOT EXISTS "server_gather_node_spawns" (
    "id"                INTEGER PRIMARY KEY AUTOINCREMENT,
    "actorClassId"      INTEGER NOT NULL,
    "uniqueId"          TEXT NOT NULL DEFAULT '',
    "zoneId"            INTEGER NOT NULL,
    "privateAreaName"   TEXT NOT NULL DEFAULT '',
    "privateAreaLevel"  INTEGER NOT NULL DEFAULT 0,
    "positionX"         REAL NOT NULL,
    "positionY"         REAL NOT NULL,
    "positionZ"         REAL NOT NULL,
    "rotation"          REAL NOT NULL DEFAULT 0.0,
    "harvestNodeId"     INTEGER NOT NULL,
    "harvestType"       INTEGER NOT NULL DEFAULT 22002
);

INSERT OR IGNORE INTO "server_gather_node_spawns"
    ("id", "actorClassId", "uniqueId", "zoneId", "positionX", "positionY", "positionZ",
     "rotation", "harvestNodeId", "harvestType")
VALUES
    (1, 4100001, 'mining_outcrop_central_thanalan_1', 180, -45.12, 12.80, 31.04, 0.0,  1001, 22002),
    (2, 4100001, 'mining_outcrop_central_thanalan_2', 180, -12.50, 14.40, 64.22, 1.57, 1002, 22002);
