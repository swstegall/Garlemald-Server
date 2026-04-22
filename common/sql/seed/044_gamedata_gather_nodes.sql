-- Gathering node templates. One row per "harvest node" definition —
-- `harvestNodeId` keys into `DummyCommand.lua`'s aim-slot build step
-- and identifies a reusable pool of possible drops for every physical
-- spawn placed in the world.
--
-- Mirrors `harvestNodeContainer` in the prior hardcoded Lua table:
--   { grade, attempts, numItems, itemKey1..itemKeyN }
-- Flattened here into fixed columns so the row is fast to SELECT. Up
-- to 11 item keys — the DummyCommand aim slider has 11 discrete slots
-- (`+5..-5` inclusive) and each slot can be bound to at most one item.
-- Empty slots are `NULL`.

DROP TABLE IF EXISTS "gamedata_gather_nodes";
CREATE TABLE IF NOT EXISTS "gamedata_gather_nodes" (
    "id"       INTEGER PRIMARY KEY,
    "grade"    INTEGER NOT NULL DEFAULT 1,
    "attempts" INTEGER NOT NULL DEFAULT 2,
    "item1"    INTEGER DEFAULT NULL,
    "item2"    INTEGER DEFAULT NULL,
    "item3"    INTEGER DEFAULT NULL,
    "item4"    INTEGER DEFAULT NULL,
    "item5"    INTEGER DEFAULT NULL,
    "item6"    INTEGER DEFAULT NULL,
    "item7"    INTEGER DEFAULT NULL,
    "item8"    INTEGER DEFAULT NULL,
    "item9"    INTEGER DEFAULT NULL,
    "item10"   INTEGER DEFAULT NULL,
    "item11"   INTEGER DEFAULT NULL
);

-- Seed two template nodes carried forward from the prior hardcoded
-- tables so the existing DummyCommand.lua keeps behaving the same
-- after the schema-driven cut. `1001` is the tutorial copper outcrop
-- (grade 2, 2 attempts, drops Rock Salt / Bone Chip / Copper Ore);
-- `1002` is the richer grade-2 node (4 attempts, five items keyed
-- 3001..3005).
INSERT OR IGNORE INTO "gamedata_gather_nodes"
    ("id", "grade", "attempts", "item1", "item2", "item3")
VALUES
    (1001, 2, 2, 1, 2, 3);

INSERT OR IGNORE INTO "gamedata_gather_nodes"
    ("id", "grade", "attempts", "item1", "item2", "item3", "item4", "item5")
VALUES
    (1002, 2, 4, 3005, 3003, 3002, 3001, 3004);
