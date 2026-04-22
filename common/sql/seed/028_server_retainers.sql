-- Ported from project-meteor-mirror/Data/sql/server_retainers.sql
-- Table: server_retainers
--
-- One row per canonical retainer template. The `actorClassId` selects
-- the retainer NPC model/race (3001101..3001175 = Limsa retainers,
-- 3002101..3002175 = Ul'dah, 3003101..3003175 = Gridania — see
-- `PopulaceRetainerManager.lua`'s `retainerIndex + 74` range). A
-- player "hires" a retainer by inserting a row in
-- `characters_retainers` linking their character id to one of these
-- templates; subsequent `SpawnMyRetainer` calls walk the join (via
-- `LIMIT 1 OFFSET retainerIndex-1`) to materialise the Nth owned
-- retainer into the zone.

CREATE TABLE IF NOT EXISTS "server_retainers" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "actorClassId" INTEGER NOT NULL,
    "cdIDOffset" INTEGER NOT NULL DEFAULT '0',
    "placeName" INTEGER NOT NULL,
    "conditions" INTEGER NOT NULL DEFAULT '0',
    "level" INTEGER NOT NULL
);

-- Three tutorial retainer templates — one per city-state so the
-- retainer-hire flow in `PopulaceRetainerManager.lua` lands something
-- regardless of which town's retainer bell the player approaches.
-- IDs are stable so tests can reference them as 1001..1003.
INSERT OR IGNORE INTO "server_retainers"
    ("id", "name", "actorClassId", "cdIDOffset", "placeName", "conditions", "level")
VALUES
    (1001, 'Wienta',   3001101, 0, 142, 0, 1),   -- Limsa Lominsa (Hyur Midlander F)
    (1002, 'Edmont',   3002101, 0, 133, 0, 1),   -- Ul'dah       (Hyur Highlander M)
    (1003, 'Lyngsath', 3003101, 0, 132, 0, 1);   -- Gridania     (Elezen Wildwood M)
