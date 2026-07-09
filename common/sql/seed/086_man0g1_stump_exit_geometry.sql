-- Man0g1 "Souls Gone Wild" — Lifemend Stump exit (SEQ_071) geometry tune
-- (Garlemald-Server #41).
--
-- SYMPTOM: the stump-exit push "did not trigger when walked towards" — it is
-- an OUTWARDS circle (fires when the player crosses OUT past its radius), so
-- walking toward the marker never fires it; you must walk away. The circle
-- was live (the spawn/stream quest-ENPC overlay enables it regardless of the
-- sequence-flip broadcast), so this is geometry, not an arming bug.
--
-- FRAGILITY: man0g1.lua SEQ_070 STUMP_TRIGGER onPush warped into this PA with
-- target:None, keeping wherever the player stood when the stump push fired.
-- That left the distance to this outward circle non-deterministic — it could
-- fire on arrival (skipping the beat) or need a long run, depending on where
-- the stump was triggered.
--
-- FIX: the Lua onPush now warps to a FIXED anchor = the recorded arrival
-- stump (-756.77, 22.77, -1092.33 = SimpleContentMan0g101 ARRIVAL_GOAL,
-- confirmed-walkable ground). This seed makes warp-in and circle CENTRE
-- coincide and shrinks the outward "leave the area" circle to a short,
-- consistent walk-out (30 -> 15). Keep in sync with man0g1.lua STUMP_TRIGGER
-- onPush.
--
-- New seed (not an edit to 074): the seed loader tracks applied filenames in
-- schema_migrations, so re-editing 074 would never re-run against the live
-- DB. UPDATEs are idempotent.

-- Shrink the outward "leave" circle (outwards stays true).
UPDATE "gamedata_actor_class" SET
    "eventConditions" = '{
  "pushWithCircleEventConditions": [
    {
      "isEnabled": "false",
      "radius": "15.0",
      "outwards": "true",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}'
WHERE "id" = 1090204;

-- Re-home the trigger onto the recorded stump so its circle centre coincides
-- with the fixed warp-in anchor set in man0g1.lua.
UPDATE "server_spawn_locations" SET
    "positionX" = -756.77,
    "positionY" = 22.77,
    "positionZ" = -1092.33
WHERE "actorClassId" = 1090204 AND "uniqueId" = 'man0g1_stump_exit_trig';
