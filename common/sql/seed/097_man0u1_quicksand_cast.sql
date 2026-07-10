-- Man0u1 "Court in the Sands" — the SEQ_000 Quicksand intro-instance
-- ambient cast (Garlemald-Server #53, branch-review follow-up).
--
-- man0u1.lua's SEQ_000 arms MOMODI plus nine ambient NPCs with
-- dedicated talk branches (processEvent000_2..000_10 — the retail
-- "objective-gated instance" crowd), but PA (175,
-- 'PrivateAreaMasterPast', 4) shipped with ONLY Momodi (seed/031 row
-- 952) — eight of the nine classes had ZERO spawn rows anywhere and
-- Otopa Pottopa only public-175/PA-3 rows. This seed makes the armed
-- cast reachable, honouring the same "an armed class must have a spawn
-- row in the area the script expects" invariant seeds 093-096 enforce
-- for the later scenes.
--
-- Positions: no marker rows exist for the intro crowd (the 110010xx
-- block starts at Momodi) — the nine are clustered around the Quicksand
-- tavern floor (Momodi PA-4 anchor -73.33/195/78.531; player warp-in
-- -75.242/195.009/74.572), rotations eyeballed toward the room centre;
-- all rows flagged for in-client tuning. Thancred stands beside Momodi
-- (the retail scene has him at the bar).
--
-- classPath fills: the eight row-less classes also ship STRIPPED in
-- seed/003 (classPath '' / propertyFlags 0 / NULL eventConditions) —
-- spawning a classPath-less class is the seed/061 client hard-crash
-- family, so they get the standard populace fill (the seed/076/093
-- recipe), guarded so a future upstream refresh with real rows wins.
--
-- Spawn ids 2452-2460: continues the man0u1 block (093 used 2400-2441,
-- 094 used 2450-2451). uniqueId prefix 'man0u1_'.
--
-- Idempotent: UPDATE guarded, INSERTs OR IGNORE on id. No schema change.

UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Populace/PopulaceStandard',
    "propertyFlags" = 19,
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
  ]
}'
WHERE "id" IN (
    1000936, -- Undaunted Adventurer
    1000937, -- Greedy Merchant
    1000938, -- Lionhearted Adventurer
    1000939, -- Spry Salesman
    1000940, -- Upbeat Adventurer
    1000941, -- Seemingly Calm Adventurer
    1000807, -- Overcompetitive Adventurer
    1000948  -- Thancred (Quicksand-scene variant; display borrows 1000010)
) AND "classPath" = '';

-- ---- PA (175, 'PrivateAreaMasterPast', 4) — the intro-instance crowd ----

INSERT OR IGNORE INTO "server_spawn_locations"
    ("id", "actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    (2452, 1000864, 'man0u1_qs_otopa_pottopa',        175, 'PrivateAreaMasterPast', 4, -70.5, 195.0, 76.0, -2.2, 0, 0, NULL),
    (2453, 1000936, 'man0u1_qs_undaunted_adventurer', 175, 'PrivateAreaMasterPast', 4, -79.0, 195.0, 70.0,  0.8, 0, 0, NULL),
    (2454, 1000937, 'man0u1_qs_greedy_merchant',      175, 'PrivateAreaMasterPast', 4, -82.5, 195.0, 76.5,  1.6, 0, 0, NULL),
    (2455, 1000938, 'man0u1_qs_lionhearted_adv',      175, 'PrivateAreaMasterPast', 4, -68.0, 195.0, 82.0, -2.6, 0, 0, NULL),
    (2456, 1000939, 'man0u1_qs_spry_salesman',        175, 'PrivateAreaMasterPast', 4, -77.5, 195.0, 84.5,  2.9, 0, 0, NULL),
    (2457, 1000940, 'man0u1_qs_upbeat_adventurer',    175, 'PrivateAreaMasterPast', 4, -71.0, 195.0, 68.5, -0.6, 0, 0, NULL),
    (2458, 1000941, 'man0u1_qs_seemingly_calm_adv',   175, 'PrivateAreaMasterPast', 4, -84.0, 195.0, 70.5,  1.2, 0, 0, NULL),
    (2459, 1000807, 'man0u1_qs_overcompetitive_adv',  175, 'PrivateAreaMasterPast', 4, -66.5, 195.0, 72.5, -1.8, 0, 0, NULL),
    (2460, 1000948, 'man0u1_qs_thancred',             175, 'PrivateAreaMasterPast', 4, -76.5, 195.0, 80.5,  2.4, 0, 0, NULL);
