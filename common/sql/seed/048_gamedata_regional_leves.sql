-- Regional leve definitions — the fieldcraft (gatherer) and
-- battlecraft (combat) counterpart to `gamedata_passivegl_craft`
-- (which covers tradecraft/crafting leves).
--
-- Shape: one row per leve, with four parallel difficulty bands each
-- describing objective target id / quantity, attempt allowance, and
-- reward. A single `leveType` discriminator selects how the target id
-- is interpreted:
--
--   leveType = 1 (FIELDCRAFT)   → objectiveTargetId is an item
--                                 catalog id (the harvested drop).
--                                 Progress increments when the player
--                                 gathers a matching item via
--                                 `Database::add_harvest_item`.
--   leveType = 2 (BATTLECRAFT)  → objectiveTargetId is a BattleNpc
--                                 actor_class_id (the killed mob).
--                                 Progress increments via the
--                                 `onKillBNpc` quest hook.
--
-- Quest id ranges (above the 112_048 `Bitstream2048` cap — leves are
-- repeatable, so they don't occupy the completed-quest bitstream):
--   * fieldcraft  : 130_001..=130_450
--   * battlecraft : 140_001..=140_450

DROP TABLE IF EXISTS "gamedata_regional_leves";
CREATE TABLE IF NOT EXISTS "gamedata_regional_leves" (
    "id"                   INTEGER PRIMARY KEY,
    "leveType"             INTEGER NOT NULL,
    "plateId"              INTEGER NOT NULL DEFAULT 0,
    "borderId"             INTEGER NOT NULL DEFAULT 0,
    "recommendedClass"     INTEGER NOT NULL DEFAULT 0,
    "issuingLocation"      INTEGER NOT NULL DEFAULT 0,
    "guildleveLocation"    INTEGER NOT NULL DEFAULT 0,
    "deliveryDisplayName"  INTEGER NOT NULL DEFAULT 0,
    "region"               INTEGER NOT NULL DEFAULT 0,

    "objectiveTargetId1"   INTEGER NOT NULL,
    "objectiveQuantity1"   INTEGER NOT NULL,
    "recommendedLevel1"    INTEGER NOT NULL DEFAULT 1,
    "rewardItemId1"        INTEGER NOT NULL DEFAULT 0,
    "rewardQuantity1"      INTEGER NOT NULL DEFAULT 0,
    "rewardGil1"           INTEGER NOT NULL DEFAULT 0,

    "objectiveTargetId2"   INTEGER NOT NULL,
    "objectiveQuantity2"   INTEGER NOT NULL,
    "recommendedLevel2"    INTEGER NOT NULL DEFAULT 1,
    "rewardItemId2"        INTEGER NOT NULL DEFAULT 0,
    "rewardQuantity2"      INTEGER NOT NULL DEFAULT 0,
    "rewardGil2"           INTEGER NOT NULL DEFAULT 0,

    "objectiveTargetId3"   INTEGER NOT NULL,
    "objectiveQuantity3"   INTEGER NOT NULL,
    "recommendedLevel3"    INTEGER NOT NULL DEFAULT 1,
    "rewardItemId3"        INTEGER NOT NULL DEFAULT 0,
    "rewardQuantity3"      INTEGER NOT NULL DEFAULT 0,
    "rewardGil3"           INTEGER NOT NULL DEFAULT 0,

    "objectiveTargetId4"   INTEGER NOT NULL,
    "objectiveQuantity4"   INTEGER NOT NULL,
    "recommendedLevel4"    INTEGER NOT NULL DEFAULT 1,
    "rewardItemId4"        INTEGER NOT NULL DEFAULT 0,
    "rewardQuantity4"      INTEGER NOT NULL DEFAULT 0,
    "rewardGil4"           INTEGER NOT NULL DEFAULT 0
);

-- -------------------------------------------------------------------
-- Fieldcraft scaffold (IDs 130_001..130_003). Three representative
-- leves, one per region, targeting items we've already seeded in the
-- gather catalog (044/045) so the progress hook fires out of the box.
-- -------------------------------------------------------------------
INSERT OR IGNORE INTO "gamedata_regional_leves"
    ("id", "leveType", "recommendedClass", "region",
     "objectiveTargetId1", "objectiveQuantity1", "recommendedLevel1", "rewardGil1",
     "objectiveTargetId2", "objectiveQuantity2", "recommendedLevel2", "rewardGil2",
     "objectiveTargetId3", "objectiveQuantity3", "recommendedLevel3", "rewardGil3",
     "objectiveTargetId4", "objectiveQuantity4", "recommendedLevel4", "rewardGil4")
VALUES
    -- La Noscea — Miner — Tin Ore (Bearded Rock pool). Region 1001.
    (130001, 1, 30, 1001,
     10001001,  5, 1,  200,
     10001001, 10, 10, 500,
     10001001, 15, 20, 1200,
     10001001, 20, 30, 2500),
    -- Black Shroud — Botanist — Walnut Log. Region 2012.
    (130002, 1, 33, 2012,
     10008007,  5, 1,  200,
     10008007, 10, 10, 500,
     10008007, 15, 20, 1200,
     10008007, 20, 30, 2500),
    -- Thanalan — Miner — Copper Ore (Black Brush pool). Region 3006.
    (130003, 1, 30, 3006,
     10001006,  5, 1,  200,
     10001006, 10, 10, 500,
     10001006, 15, 20, 1200,
     10001006, 20, 30, 2500);

-- -------------------------------------------------------------------
-- Battlecraft scaffold (IDs 140_001..140_003). Actor-class ids are
-- placeholders in the `5_000_xxx` range matching Meteor's BattleNpc
-- class-id convention; swap in canonical ids once a
-- `server_battlenpc_*` seed pool lands. The hook path only cares
-- that the id matches the one the BattleNpc's actor class carries.
-- -------------------------------------------------------------------
INSERT OR IGNORE INTO "gamedata_regional_leves"
    ("id", "leveType", "recommendedClass", "region",
     "objectiveTargetId1", "objectiveQuantity1", "recommendedLevel1", "rewardGil1",
     "objectiveTargetId2", "objectiveQuantity2", "recommendedLevel2", "rewardGil2",
     "objectiveTargetId3", "objectiveQuantity3", "recommendedLevel3", "rewardGil3",
     "objectiveTargetId4", "objectiveQuantity4", "recommendedLevel4", "rewardGil4")
VALUES
    -- La Noscea — Aldgoat cull (placeholder class id).
    (140001, 2, 2, 1001,
     5000035, 3,  1,  300,
     5000035, 5,  10, 700,
     5000035, 8,  20, 1800,
     5000035, 12, 30, 3500),
    -- Black Shroud — Funguar fumigation (placeholder class id).
    (140002, 2, 2, 2012,
     5000076, 3,  1,  300,
     5000076, 5,  10, 700,
     5000076, 8,  20, 1800,
     5000076, 12, 30, 3500),
    -- Thanalan — Drake extermination (placeholder class id).
    (140003, 2, 2, 3006,
     5000091, 3,  1,  300,
     5000091, 5,  10, 700,
     5000091, 8,  20, 1800,
     5000091, 12, 30, 3500);
