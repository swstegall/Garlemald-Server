-- Per-item drop metadata for gathering nodes. One row per "item key"
-- referenced from `gamedata_gather_nodes.item1..item11`.
--
-- Column meanings mirror the old hardcoded `harvestNodeItems` Lua table:
--   itemCatalogId — the 1.x catalog id that lands in the player bag
--                   on a successful strike (e.g. 10001006 = Copper Ore)
--   remainder     — node HP pool at the start of this item's strike
--                   phase. `DummyCommand.lua` decrements by 20 per swing.
--                   Classic values: 40 / 60 / 70 / 80 (labelled A..D).
--   aim           — 0..100 slider position that selects this item when
--                   the player commits. Rounds down to one of 11 slots
--                   (aim/10 + 1) in the minigame.
--   sweetspot     — 0..100 power-bar target for the strike phase.
--                   `powerRange` (±10) is the width of the "hit" band.
--   maxYield      — maximum quantity granted on a perfect strike.

DROP TABLE IF EXISTS "gamedata_gather_node_items";
CREATE TABLE IF NOT EXISTS "gamedata_gather_node_items" (
    "id"             INTEGER PRIMARY KEY,
    "itemCatalogId"  INTEGER NOT NULL,
    "remainder"      INTEGER NOT NULL DEFAULT 80,
    "aim"            INTEGER NOT NULL DEFAULT 50,
    "sweetspot"      INTEGER NOT NULL DEFAULT 30,
    "maxYield"       INTEGER NOT NULL DEFAULT 1
);

INSERT OR IGNORE INTO "gamedata_gather_node_items"
    ("id", "itemCatalogId", "remainder", "aim", "sweetspot", "maxYield")
VALUES
    (1,    10009104, 70, 30, 30, 4),   -- Rock Salt
    (2,    10006001, 80, 10, 30, 4),   -- Bone Chip
    (3,    10001006, 80, 20, 30, 3),   -- Copper Ore
    (3001, 10001003, 80, 50, 30, 3),
    (3002, 10001006, 70, 70, 10, 4),
    (3003, 10001005, 80, 90, 70, 1),
    (3004, 10009104, 40, 10, 100, 2),
    (3005, 10001007, 40,  0, 30, 1);
