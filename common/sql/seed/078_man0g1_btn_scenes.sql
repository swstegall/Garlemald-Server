-- Man0g1 — the Greatloam Growery (BTN guild) scene casts + Fane desk
-- nudge (Garlemald-Server #41, live 2026-07-06: "the Botanist guild
-- instance loads with none of the NPCs" — SEQ_050 softlock).
--
-- Findings:
--   (1) The SEQ_050 dance-practice instance (155 PrivateAreaMasterPast
--       1, warp anchor -223.792, 12, -1498.369) and the SEQ_055
--       kids-chat instance (PA 2, anchor -231.474, 12, -1500.86) have
--       ZERO cast rows in any upstream — the six kids exist only in a
--       DIFFERENT quest's scene (etc5g1_* rows, 206 PA 5) and Opyltyl
--       only in public 206. Same dead-upstream family as the CNJ cast.
--       Empty instance = the emote drill can never complete = softlock.
--   (2) Fufucha (1000237) and Nicollaux (1000409) ship classPath-EMPTY
--       (the established client-crash class) — filled FIRST below.
--   (3) Corroboration: Opyltyl public at (-205.68, -1454.72) in 206
--       proves the Growery is the x~-200..-213 / z~-1455..-1487
--       cluster in the retail pcap table (its district label there is
--       wrong); the quest's own kids-exit warp (-209.8, -1477.4) and
--       BTN trigger station (-202.9, -1477.5) agree.
--
-- Positions are provisional offsets around each scene's own warp
-- anchor (the CNJ-scene caveat); the emote drill spreads the kids in a
-- wide arc (retail Part-4 footage: six kids spaced across the practice
-- grounds, each teaching one emote), the SEQ_055 set clusters them
-- conspiring near the KIDS_TRIGGER circle (seed/074: -240, 12, -1510).

-- ---- (1) classPath fills FIRST (crash guard) --------------------------

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
WHERE "id" IN (1000237, 1000409) AND "classPath" = '';

-- ---- (2) SEQ_050 dance-practice cast (155 PA 1) -----------------------

INSERT OR IGNORE INTO "server_spawn_locations"
    ("actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    (1000236, 'man0g1_opyltyl_drill',   155, 'PrivateAreaMasterPast', 1, -221.0, 12.0, -1495.0,  2.60, 0, 1015, null),
    (1000237, 'man0g1_fufucha_drill',   155, 'PrivateAreaMasterPast', 1, -224.0, 12.0, -1506.0,  0.40, 0, 1015, null),
    (1000410, 'man0g1_aunillie_drill',  155, 'PrivateAreaMasterPast', 1, -230.0, 12.0, -1508.0,  0.90, 0, 0, null),
    (1000409, 'man0g1_nicollaux_drill', 155, 'PrivateAreaMasterPast', 1, -220.0, 12.0, -1510.0, -0.30, 0, 0, null),
    (1000239, 'man0g1_sansa_drill',     155, 'PrivateAreaMasterPast', 1, -215.0, 12.0, -1504.0, -1.10, 0, 0, null),
    (1000238, 'man0g1_powle_drill',     155, 'PrivateAreaMasterPast', 1, -228.0, 12.0, -1513.0,  0.60, 0, 0, null),
    (1000412, 'man0g1_ryd_drill',       155, 'PrivateAreaMasterPast', 1, -233.0, 12.0, -1502.0,  1.30, 0, 0, null),
    (1000411, 'man0g1_elyn_drill',      155, 'PrivateAreaMasterPast', 1, -217.0, 12.0, -1516.0, -0.60, 0, 0, null);

-- ---- (3) SEQ_055 kids-chat cast (155 PA 2) ----------------------------

INSERT OR IGNORE INTO "server_spawn_locations"
    ("actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    (1000236, 'man0g1_opyltyl_chat',    155, 'PrivateAreaMasterPast', 2, -228.0, 12.0, -1498.0,  2.20, 0, 1015, null),
    (1000237, 'man0g1_fufucha_chat',    155, 'PrivateAreaMasterPast', 2, -229.0, 12.0, -1504.0,  0.80, 0, 1015, null),
    (1000410, 'man0g1_aunillie_chat',   155, 'PrivateAreaMasterPast', 2, -237.5, 12.0, -1507.0,  0.70, 0, 0, null),
    (1000409, 'man0g1_nicollaux_chat',  155, 'PrivateAreaMasterPast', 2, -234.5, 12.0, -1509.5,  1.20, 0, 0, null),
    (1000239, 'man0g1_sansa_chat',      155, 'PrivateAreaMasterPast', 2, -236.0, 12.0, -1511.0,  0.30, 0, 0, null),
    (1000238, 'man0g1_powle_chat',      155, 'PrivateAreaMasterPast', 2, -238.5, 12.0, -1509.0, -0.20, 0, 0, null),
    (1000412, 'man0g1_ryd_chat',        155, 'PrivateAreaMasterPast', 2, -233.0, 12.0, -1506.5,  1.60, 0, 0, null),
    (1000411, 'man0g1_elyn_chat',       155, 'PrivateAreaMasterPast', 2, -239.5, 12.0, -1512.5,  0.10, 0, 0, null);

-- ---- (4) Fane desk nudge (live report: pair stood mid-room) -----------

-- Push Soileine + Hetzkin ~6u deeper (behind the counter line, facing
-- back toward the entrance). Still provisional — precision pass =
-- stand-and-mark (read the player's own 0x00CA position at the desired
-- spot from the log).
UPDATE "server_spawn_locations" SET "positionX" = -356.5, "positionY" = 6.25, "positionZ" = -1708.5, "rotation" = -2.37
WHERE "uniqueId" = 'man0g1_soileine';
UPDATE "server_spawn_locations" SET "positionX" = -353.5, "positionY" = 6.25, "positionZ" = -1709.3, "rotation" = -2.37
WHERE "uniqueId" = 'man0g1_hetzkin_fane';
