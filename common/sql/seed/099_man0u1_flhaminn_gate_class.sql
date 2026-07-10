-- Man0u1 "Court in the Sands" — gate-muster F'lhaminn class split
-- (Garlemald-Server: man0u1 dual-spawn fix).
--
-- Root cause: `quest:SetENpc(class, ...)` and the whole engine pipeline
-- behind it (QuestSetEnpc command → dispatcher claim → streaming
-- overlay) are class-keyed and zone-agnostic, so the STAGE spawn 2433
-- (`man0u1_flhaminn_stage`, zone 209, Concern stage) and the GATE spawn
-- 2435 (`man0u1_flhaminn_gate`, zone 170, Gate of Nald) of class
-- 1000038 always arm together. Live run 2026-07-10: at SEQ_060 the
-- player talked to the stage copy inside the Miner's Guild and got the
-- gate-side wait texts (144-146) with a `!` icon — retail scopes the
-- two stations apart (marker 11001014 is the gate; the stage is empty
-- flavour until SEQ_075).
--
-- Fix: mint a gate-variant class and repoint spawn 2435 at it, so the
-- script can arm SEQ_060 (gate) and SEQ_075 (stage) independently.
-- Id 1099100 verified absent from BOTH gamedata_actor_class and
-- gamedata_actor_appearance (populace-range max is 1099069) — a truly
-- free id, so INSERT OR IGNORE cannot silently no-op on a collision
-- (the seed/082 gotcha). Precedents: seed/074 (net-new class +
-- appearance rows for the escort kids), seed/093 (in-quest F'lhaminn
-- scene variant 1000842 sharing displayNameId 1900054).
--
-- Client safety: nameplate text is wire-sent from displayNameId,
-- classPath rides the script-bind tail, and the appearance block is
-- server-sent — the client never looks anything up by class id, so a
-- server-only id renders as a normal F'lhaminn.
--
-- Idempotent: INSERTs OR IGNORE on id, UPDATE guarded on the old class.
-- No schema change.

-- ---- (1) Gate-variant class (standard populace recipe, seed/093) -----

INSERT OR IGNORE INTO "gamedata_actor_class"
    ("id", "classPath", "displayNameId", "propertyFlags", "eventConditions") VALUES
    (1099100, '/Chara/Npc/Populace/PopulaceStandard', 1900054, 19, '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}');

-- ---- (2) Appearance: byte-identical clone of the base F'lhaminn -----

-- INSERT…SELECT (id swapped) instead of literal VALUES so a future
-- upstream refresh of 1000038's look carries over on fresh databases.
INSERT OR IGNORE INTO "gamedata_actor_appearance"
    ("id", "base", "size", "hairStyle", "hairHighlightColor", "hairVariation",
     "faceType", "characteristics", "characteristicsColor", "faceEyebrows",
     "faceIrisSize", "faceEyeShape", "faceNose", "faceFeatures", "faceMouth",
     "ears", "hairColor", "skinColor", "eyeColor", "voice", "mainHand",
     "offHand", "spMainHand", "spOffHand", "throwing", "pack", "pouch",
     "head", "body", "legs", "hands", "feet", "waist", "neck", "leftEar",
     "rightEar", "leftIndex", "rightIndex", "leftFinger", "rightFinger")
SELECT
    1099100, "base", "size", "hairStyle", "hairHighlightColor", "hairVariation",
    "faceType", "characteristics", "characteristicsColor", "faceEyebrows",
    "faceIrisSize", "faceEyeShape", "faceNose", "faceFeatures", "faceMouth",
    "ears", "hairColor", "skinColor", "eyeColor", "voice", "mainHand",
    "offHand", "spMainHand", "spOffHand", "throwing", "pack", "pouch",
    "head", "body", "legs", "hands", "feet", "waist", "neck", "leftEar",
    "rightEar", "leftIndex", "rightIndex", "leftFinger", "rightFinger"
FROM "gamedata_actor_appearance" WHERE "id" = 1000038;

-- ---- (3) Repoint the gate spawn at the new class ---------------------

UPDATE "server_spawn_locations" SET "actorClassId" = 1099100
WHERE "id" = 2435 AND "actorClassId" = 1000038;
