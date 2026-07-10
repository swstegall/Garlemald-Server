-- Man0u1 "Court in the Sands" — quest push-triggers (Garlemald-Server #53).
--
--   (1) Gate of Nald muster trigger 1090285 (SEQ_060 duty-join push,
--       public zone 170): a net-new pushWithCircle class on the seed/074
--       trigger recipe (PopulaceStandard + a disabled-until-armed circle;
--       the quest arms it with SetENpc(GATE_OF_NALD_TRIGGER, QFLAG_PUSH,
--       false, true)). 1090285 ships stripped in seed/003 (classPath ''),
--       same as 074's 1090200-1090204 band.
--   (2) Arena-gate trigger 1090283 (SEQ_015 subseqGLD==2, "Now go and
--       wait within."): Slerp Lederp's dev capture leaks this class as
--       the ">Pushed GLD!" push. Its seed/003 row already carries
--       talkDefault + noticeEvent but NO pushWithCircle — merge one in
--       (the seed/074 1090046 keep-existing-conditions recipe), then
--       spawn a PA (209/5) copy inside the lobby at marker 11001007.
--   (3) Ward door 1090119 (ObjectEventDoor; Slerp's SEQ 90/95/100 push
--       class): seed/003 ships it with NULL eventConditions, so no push
--       could ever fire. Give it the man0u0 exit-door recipe (1099046:
--       empty talk + noticeEvent + a radius-6 pushDefault circle), with
--       "isEnabled": "false" so both the public door (seed/031 row 250)
--       and the PA copy (seed/093 row 2431) stay inert until the quest
--       arms them (SetENpc(WARD_DOOR, QFLAG_PUSH, false, true) at
--       SEQ_090/100/105). classPath/propertyFlags untouched.
--
-- Spawn ids 2450-2451 continue seed/093's clean explicit block.
--
-- Idempotent: UPDATEs guarded, INSERTs OR IGNORE on id. No schema change.

-- ---- (1) Gate of Nald muster trigger (net-new class) -----------------

UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Populace/PopulaceStandard',
    "propertyFlags" = 1,
    "eventConditions" = '{
  "pushWithCircleEventConditions": [
    {
      "isEnabled": "false",
      "radius": "10.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}'
WHERE "id" = 1090285 AND "classPath" = '';

INSERT OR IGNORE INTO "server_spawn_locations"
    ("id", "actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    -- Marker 11001014 (the muster point just outside the Gate of Nald;
    -- y=183.0 interpolated like the F'lhaminn row beside it — FLAG FOR
    -- IN-CLIENT TUNING).
    (2450, 1090285, 'man0u1_gate_trig', 170, '', 0, -34.54, 183.0, -68.88, 0, 0, 0, null);

-- ---- (2) Arena-gate trigger: merge a push circle into 1090283 --------

-- Existing seed/003 conditions (talkDefault + noticeEvent) are KEPT
-- verbatim; only the circle is new. Guarded so it applies once and so a
-- future upstream refresh that already carries a circle wins.
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
WHERE "id" = 1090283 AND "eventConditions" NOT LIKE '%pushWithCircle%';

INSERT OR IGNORE INTO "server_spawn_locations"
    ("id", "actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    -- Marker 11001007 — inside the lobby, the "wait within" gate the
    -- SEQ_015 duty-join push fires from. (The class's public row 257 at
    -- the coliseum entrance keeps its dormant circle — the quest only
    -- arms it while the player is inside the lobby PA.)
    (2451, 1090283, 'man0u1_arena_gate_trig', 209, 'PrivateAreaMasterPast', 5, -187.23, 192.5, 219.73, 0, 0, 0, null);

-- ---- (3) Ward door 1090119: the 1099046 exit-door condition recipe ---

UPDATE "gamedata_actor_class" SET
    "eventConditions" = '{
  "talkEventConditions": [],
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}'
WHERE "id" = 1090119 AND "eventConditions" IS NULL;
