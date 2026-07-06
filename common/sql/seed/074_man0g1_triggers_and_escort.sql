-- Man0g1 "Souls Gone Wild" — quest push-triggers + the SEQ_065 White
-- Wolf Gate escort duty (Garlemald-Server #41).
--
-- Upstream (pms AND MeteorReborn) never spawned ANY of the five man0g1
-- trigger classes (1090200-1090204 sit in gamedata with empty
-- classPath/eventConditions and zero spawn rows), so every proximity
-- push from the CNJ-guild Echo through the stump exit was dead — on
-- top of the stubbed escort the issue tracks. This seed:
--   (1) fills the trigger classes with the man0l1 push-trigger recipe
--       (seed/056's 1090004: PopulaceStandard + a disabled-until-armed
--       pushWithCircle circle; SetENpc(..., QFLAG_PUSH, ...) enables it);
--   (2) spawns them at their quest stations;
--   (3) seeds the escort cast: Powle + Sansa as FighterAlly-scripted
--       battle npcs wearing the kids' real names + appearances, and a
--       zone-150 ankle-biter group (same chigoe class as the man0l1
--       escort — the same mob is on film in the Gridania duty).
--
-- PROVISIONAL COORDINATES: every row marked "provisional" stands in
-- until the recorded White-Wolf-Gate→Lifemend-Stump walk lands (the
-- man0l1 round-7f 0x00CA recipe) — the gate trigger, the duty trail
-- (SimpleContentMan0g101.lua TRAIL), and the ambush spawn points all
-- re-home from that recording. Anchors used meanwhile: BTN guild
-- station (-202.91, 18.09, -1477.51) = pmeteor's only spawned man0g1
-- trigger (server_eventnpc_spawn_locations row 553, town zone
-- translated); Lifemend Stump ≈ (-800, 20, -1050) zone 150 (classic
-- wiki quest_marker table); the SEQ_070 stump warp lands at
-- (-770.197, 23, -1086.209).
--
-- UPDATEs are idempotent and only fill rows the 003 seed left stripped;
-- INSERTs are OR IGNORE. No schema change.

-- ---- (1) Trigger actor classes -------------------------------------

-- CNJ_TRIG — Stillglade Fane Echo push (SEQ_015, subseqCNJ==1).
UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Populace/PopulaceStandard',
    "propertyFlags" = 1,
    "eventConditions" = '{
  "pushWithCircleEventConditions": [
    {
      "isEnabled": "false",
      "radius": "12.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}'
WHERE "id" = 1090200 AND "classPath" = '';

-- KIDS_TRIGGER — the dance-practice grounds chat (SEQ_055).
UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Populace/PopulaceStandard',
    "propertyFlags" = 1,
    "eventConditions" = '{
  "pushWithCircleEventConditions": [
    {
      "isEnabled": "false",
      "radius": "12.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}'
WHERE "id" = 1090201 AND "classPath" = '';

-- GATE_TRIGGER — White Wolf Gate duty-join push (SEQ_060). Radius 25
-- like the man0l1 gate trigger (seed/063: sized to the seamless-
-- boundary drop distance; re-verify once the real gate coords land).
UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Populace/PopulaceStandard',
    "propertyFlags" = 1,
    "eventConditions" = '{
  "pushWithCircleEventConditions": [
    {
      "isEnabled": "false",
      "radius": "25.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}'
WHERE "id" = 1090202 AND "classPath" = '';

-- STUMP_TRIGGER — walk to the Lifemend Stump (SEQ_070).
UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Populace/PopulaceStandard',
    "propertyFlags" = 1,
    "eventConditions" = '{
  "pushWithCircleEventConditions": [
    {
      "isEnabled": "false",
      "radius": "12.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}'
WHERE "id" = 1090203 AND "classPath" = '';

-- STUMP_EXIT_TRIGGER — leave the stump scene (SEQ_071). Outwards
-- circle: the push fires when the player LEAVES the area.
UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Populace/PopulaceStandard',
    "propertyFlags" = 1,
    "eventConditions" = '{
  "pushWithCircleEventConditions": [
    {
      "isEnabled": "false",
      "radius": "30.0",
      "outwards": "true",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}'
WHERE "id" = 1090204 AND "classPath" = '';

-- BTN_TRIGGER (1090046) — return to the Greatloam Growery (SEQ_072).
-- Upstream class carries talk/notice conditions only; the quest arms it
-- as a PUSH (SetENpc(BTN_TRIGGER, QFLAG_PUSH, false, true)), so add the
-- circle alongside the existing conditions. Guarded so it applies once.
UPDATE "gamedata_actor_class" SET
    "eventConditions" = '{
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
  ],
  "pushWithCircleEventConditions": [
    {
      "isEnabled": "false",
      "radius": "12.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}'
WHERE "id" = 1090046 AND "eventConditions" NOT LIKE '%pushWithCircle%';

-- ---- (2) Trigger spawn locations -----------------------------------

INSERT OR IGNORE INTO "server_spawn_locations"
    ("actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    -- CNJ Echo push, public town outside Stillglade Fane (provisional —
    -- mirrors the private-area scene entry the push warps into).
    (1090200, 'man0g1_cnj_trig', 155, '', 0, -353.05, 6.25, -1697.39, 0, 0, 0, null),
    -- Kids' practice-grounds chat, Greatloam Growery scene area
    -- (provisional — near the SEQ_055 warp-in at -231.474/-1500.86).
    (1090201, 'man0g1_kids_trig', 155, 'PrivateAreaMasterPast', 2, -240.0, 12.0, -1510.0, 0, 0, 0, null),
    -- White Wolf Gate duty-join push, public town (PROVISIONAL stand-in
    -- southwest of the Growery — re-home from the recorded walk).
    (1090202, 'man0g1_gate_trig', 155, '', 0, -235.0, 18.0, -1455.0, 0, 0, 0, null),
    -- Lifemend Stump walk-up, stump scene private area (provisional).
    (1090203, 'man0g1_stump_trig', 150, 'PrivateAreaMasterPast', 0, -790.0, 21.0, -1060.0, 0, 0, 0, null),
    -- Stump exit (outwards circle around the scene anchor; provisional).
    (1090204, 'man0g1_stump_exit_trig', 150, 'PrivateAreaMasterPast', 1, -770.0, 23.0, -1086.0, 0, 0, 0, null),
    -- Greatloam Growery return — pmeteor's own station for this trigger
    -- (server_eventnpc_spawn_locations row 553, zone translated).
    (1090046, 'man0g1_btn_trig', 155, '', 0, -202.91, 18.09, -1477.51, -1.42, 0, 0, null);

-- ---- (3) Escortees: Powle + Sansa as battle-npc allies ---------------

-- New monster-band classes on the PROVEN FighterAllyOpeningAttacker
-- classPath (its class script supplies the battleCommon tuple — a
-- battle npc without one crashes the client's DepictionJudge; the
-- man0l1 chigoe lesson). Display names are the kids' REAL populace
-- name rows (1000238 → 1000029 "Powle", 1000239 → 1100025 "Sansa" —
-- note the non-standard mapping in gamedata_actor_class).
INSERT OR IGNORE INTO "gamedata_actor_class" ("id", "classPath", "displayNameId", "propertyFlags", "eventConditions") VALUES
    ('2290020', '/Chara/Npc/Monster/Fighter/FighterAllyOpeningAttacker', '1000029', '0', null);
INSERT OR IGNORE INTO "gamedata_actor_class" ("id", "classPath", "displayNameId", "propertyFlags", "eventConditions") VALUES
    ('2290021', '/Chara/Npc/Monster/Fighter/FighterAllyOpeningAttacker', '1100025', '0', null);

-- Appearance rows cloned from the kids' populace classes (positional
-- copies of seed/002's 1000238/1000239 rows with the id swapped).
INSERT OR IGNORE INTO "gamedata_actor_appearance" VALUES
    (2290020, 21, 2, 0, 0, 0, 21, 0, 0, 0, 0, 0, 0, 0, 0, 0, 14, 9, 16, 0, 0, 0, 0, 0, 0, 0, 0, 1728, 1728, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "gamedata_actor_appearance" VALUES
    (2290021, 21, 2, 0, 0, 0, 31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 12, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 2752, 2752, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);

-- Pools: kids ride the Fighter genus like Sisipu (they never fight —
-- the content script scripts no engage for them; the combat columns
-- are inert defaults). Ankle biters REUSE pool 12 (class 2205603,
-- seed/056) — pools are zone-free.
INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (13, 2290020, 'escort_powle', 29, 2, 1, 4200, 1, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (14, 2290021, 'escort_sansa', 29, 2, 1, 4200, 1, 0, 0, 0, 0, 0);

-- Groups: allies (allegiance 1) with modest kid HP — either kid dying
-- fails the duty (content-script live-roster check); ankle biters at
-- the man0l1 escort's retail-calibrated band (lvl 5-7, hp 60 — "die to
-- 2-3 early-rank weaponskills" per the retail recording).
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (13, 13, 'escort_powle', 1, 1, 0, 200, 0, 0, 1, 1, 0, 0, '', 0, 150);
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (14, 14, 'escort_sansa', 1, 1, 0, 200, 0, 0, 1, 1, 0, 0, '', 0, 150);
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (15, 12, 'ankle_biter_gridania', 5, 7, 0, 60, 0, 0, 0, 1, 0, 0, '', 0, 150);

-- Spawn points (SimpleContentMan0g101.lua onCreate spawns these ids).
-- PROVISIONAL: kids at the duty warp-in anchor, biters along the
-- placeholder arc — ALL re-home from the recorded walk (the seed/072
-- one-per-ambush-point recipe).
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (34, 'escort_powle', 13, -702.0, 21.0, -1003.0, 2.4),
    (35, 'escort_sansa', 14, -699.0, 21.0, -1005.0, 2.4),
    (36, 'ankle_biter', 15, -715.0, 21.0, -1012.0, 0.0),
    (37, 'ankle_biter', 15, -724.0, 21.2, -1020.0, 0.0),
    (38, 'ankle_biter', 15, -733.0, 21.5, -1027.0, 0.0),
    (39, 'ankle_biter', 15, -741.0, 21.7, -1035.0, 0.0),
    (40, 'ankle_biter', 15, -747.0, 22.0, -1043.0, 0.0),
    (41, 'ankle_biter', 15, -752.0, 22.0, -1048.0, 0.0);

-- ---- (4) Missing private-area registrations -------------------------

-- The man0g1 scene warps address 155 PrivateAreaMasterPast levels 0/3/4
-- (CNJ Echo, Burchard scene, wounded-rangers scene) and 150 levels 0/1
-- (Lifemend Stump scene + interior) — NONE of which exist upstream
-- (pms ships only 155 levels 1-2; zone 150 has no private areas at
-- all): the whole quest tail was unaddressable, matching the
-- never-spawned triggers. Register them so the ported warps land.
-- Music: 155 rows copy the sibling level-1 value (51); 150 rows carry
-- zone 150's own day/night/battle set (52/52/13, seed/033). Ids
-- omitted — AUTOINCREMENT avoids colliding with future upstream ports.
INSERT OR IGNORE INTO "server_zones_privateareas" ("parentZoneId", "className", "privateAreaName", "privateAreaType", "dayMusic", "nightMusic", "battleMusic") VALUES
    ('155', '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', '0', '51', '0', '0'),
    ('155', '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', '3', '51', '0', '0'),
    ('155', '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', '4', '51', '0', '0'),
    ('150', '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', '0', '52', '52', '13'),
    ('150', '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', '1', '52', '52', '13');
