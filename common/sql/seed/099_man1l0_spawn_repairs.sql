-- Man1l0 "Legends Adrift" — spawn repairs (Garlemald-Server #48).
--
-- (1) Restore the SEQ_070 seafield push trigger. pmeteor's
-- server_eventnpc_spawn_locations row 2090 (man1l0_seafld1_push: class
-- 1090082, zone 128 'sea0Field01' at 218.58 / 21.025 / 1176.56) never made
-- it into the seed set — seed/059 restored the quest's other rows
-- (2068-2089 and 2091-2108, skipping exactly 2090), and seed/031's public
-- pass missed it too. With no spawn there is nothing for SEQ_070's
-- SetENpc(TRIGGER_SEAFLD, QFLAG_PUSH, false, true) to arm, so the quest
-- soft-locks at "head to a spot in Lower La Noscea". The position matches
-- the client's SEQ_070 journal marker 11000307 (218.58 / 1176.56) exactly.
--
-- Class 1090082 also ships stripped in seed/003 (classPath '',
-- propertyFlags 0), and the spawn pass skips empty-classPath rows (the
-- seed/098 engine guard) — restore pmeteor's class shape too
-- (PopulaceStandard, flags 1, matching the sibling quest triggers
-- 1090080/1090081/1090083/1090084). Its eventConditions (noticeEvent + a
-- disabled radius-6 pushDefault circle) were already fixed by seed/057.
--
-- (2) Give ASSESSOR1 (1000120) a real spot in the gate-echo PA. Row 2107
-- (man1l0_echo3_assessor2) ships at 0/0/0 — a pmeteor placeholder carried
-- into seed/059 verbatim — so the SEQ_100 talk owner of client text 159
-- stands ~820 units outside the scene and processEvent2000_2 is
-- unreachable in live play. No retail position survives in any source
-- (pmeteor, MeteorReborn, captures, recordings), so the spot is
-- synthesized beside his reception partner ASSESSOR2 (row 2093 at
-- -782.897 / 12.9 / 199.012, anim 1015), same floor, same idle pose.
--
-- Idempotent: UPDATEs guarded on the broken shapes, INSERT OR IGNORE on
-- id. No schema change.

-- ---- (1) seafield push trigger ---------------------------------------

UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Populace/PopulaceStandard',
    "propertyFlags" = 1
WHERE "id" = 1090082 AND "classPath" = '';

INSERT OR IGNORE INTO "server_spawn_locations"
    ("id", "actorClassId", "uniqueId", "zoneId", "privateAreaName",
     "privateAreaLevel", "positionX", "positionY", "positionZ", "rotation",
     "actorState", "animationId", "customDisplayName")
VALUES
    (2090, 1090082, 'man1l0_seafld1_push', 128, '', 0,
     218.58, 21.025, 1176.56, 0, 0, 0, NULL);

-- ---- (2) ASSESSOR1 gate-echo position --------------------------------

UPDATE "server_spawn_locations" SET
    "positionX" = -780.9,
    "positionY" = 12.9,
    "positionZ" = 199.8,
    "rotation" = 0.052,
    "animationId" = 1015
WHERE "id" = 2107
  AND "uniqueId" = 'man1l0_echo3_assessor2'
  AND "positionX" = 0 AND "positionY" = 0 AND "positionZ" = 0;
