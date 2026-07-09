-- Man0g1 "Souls Gone Wild" — SEQ_090 Lancers' Guild scene cast (PA3)
-- (Garlemald-Server #41).
--
-- The room loaded EMPTY in garlemald (scene dialogue played, ZERO NPCs):
-- the SEQ_090 cast was never spawned in zone 155 PrivateAreaMasterPast
-- level 3 (the SEQ_085 Burchard-talk warp destination), and six of the
-- eleven cast classes ship classPath-EMPTY — a classPath-less spawn crashes
-- the 1.x client's DepictionJudge (the repeated escort/CNJ lesson), so they
-- were never placed. This seed:
--   (1) fills the six stripped classes with the populace recipe (+ a
--       talkDefault condition so their SEQ_090 flavor talks can fire);
--   (2) spawns all eleven in PA3, arranged to the reference playthrough
--       (youtube A2sSGS6RVOw + user frames): Burchard + Nuala stand as the
--       commanders, survivors (T'kebbe / Farrimond recount the attack)
--       stand around them, and the worst-wounded lancers lie collapsed.
--
-- PROVISIONAL COORDINATES: positions are read off the reference frames
-- relative to the PA3 warp anchor (176.13, 27.5, -1581.84; man0g1.lua
-- SEQ_085 Burchard onTalk) — no in-client coordinate readout exists for the
-- video, so fine-tune by stand-and-mark. Collapsed pose = animationId 1007
-- (the man0g0 Yda lying pose); standing scene pose = 0. The 155 PA level 3
-- was registered in seed/074.
--
-- New seed (not an edit to 074): the loader tracks applied filenames, so a
-- committed seed never re-runs against the live DB. UPDATE/INSERT idempotent.

-- (1) Fill the six stripped cast classes (crash guard + clickable).
UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Populace/PopulaceStandard',
    "propertyFlags" = 19,
    "eventConditions" = '{"talkEventConditions":[{"unknown1":4,"unknown2":0,"conditionName":"talkDefault"}]}'
WHERE "id" IN (1000015, 1000733, 1000734, 1000735, 1000738, 1000741) AND "classPath" = '';

-- (2) Spawn the SEQ_090 cast in zone 155 PrivateAreaMasterPast level 3.
INSERT OR IGNORE INTO "server_spawn_locations"
    ("actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    -- Commanders (standing, center-back — Burchard is the SEQ_090 talk target).
    (1000243, 'man0g1_pa3_burchard',     155, 'PrivateAreaMasterPast', 3, 191.0, 27.5, -1579.5, 2.0, 0, 0, null),
    (1000681, 'man0g1_pa3_nuala',        155, 'PrivateAreaMasterPast', 3, 187.0, 27.5, -1580.0, 2.0, 0, 0, null),
    -- Standing survivors (T'kebbe + Farrimond recount the attack in the dialogue).
    (1000015, 'man0g1_pa3_tkebbe',       155, 'PrivateAreaMasterPast', 3, 190.0, 27.5, -1581.5, 2.0, 0, 0, null),
    (1000017, 'man0g1_pa3_farrimond',    155, 'PrivateAreaMasterPast', 3, 188.0, 27.5, -1578.0, 2.2, 0, 0, null),
    (1000734, 'man0g1_pa3_langloisiert', 155, 'PrivateAreaMasterPast', 3, 184.0, 27.5, -1581.5, 1.6, 0, 0, null),
    (1000733, 'man0g1_pa3_turstin',      155, 'PrivateAreaMasterPast', 3, 192.0, 27.5, -1578.0, 2.4, 0, 0, null),
    (1000683, 'man0g1_pa3_cecilia',      155, 'PrivateAreaMasterPast', 3, 192.0, 27.5, -1581.5, 2.3, 0, 0, null),
    (1000735, 'man0g1_pa3_helbhanth',    155, 'PrivateAreaMasterPast', 3, 183.0, 27.5, -1582.0, 1.6, 0, 0, null),
    -- Collapsed / wounded (on the floor, animationId 1007).
    (1000682, 'man0g1_pa3_mansel',       155, 'PrivateAreaMasterPast', 3, 188.0, 27.5, -1577.5, 0.0, 0, 1007, null),
    (1000741, 'man0g1_pa3_jijimaya',     155, 'PrivateAreaMasterPast', 3, 183.0, 27.5, -1580.5, 0.0, 0, 1007, null),
    (1000738, 'man0g1_pa3_pasdevillet',  155, 'PrivateAreaMasterPast', 3, 186.0, 27.5, -1578.5, 0.0, 0, 1007, null);

-- (3) SEQ_095 re-instances the SAME wounded-guild scene in PA level 4 (talk
-- Nuala → SEQ_100 → 206). The SEQ_095 arm casts a subset —
-- Nuala/Burchard/T'kebbe/Farrimond/Cecilia standing + Mansel/Jijimaya
-- collapsed — placed at the PA3 positions (the reference frame that targets
-- Nuala shows the identical layout). PROVISIONAL like PA3; fine-tune on the
-- Nuala beat by stand-and-mark.
INSERT OR IGNORE INTO "server_spawn_locations"
    ("actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    -- Nuala + Burchard address the room from the back-centre platform;
    -- survivors cluster mid/front (T'kebbe/Farrimond centre, Cecilia/Mansel
    -- toward the entry doors), Jijimaya off to the far left, wounded on the
    -- floor. The player enters at the front (SEQ_090 warp coords) and walks
    -- back to Nuala (the SEQ_095 talk target). Layout from the reference
    -- frames (2026-07-07); coords frame-estimated, fine-tune by stand-and-mark.
    (1000681, 'man0g1_pa4_nuala',      155, 'PrivateAreaMasterPast', 4, 189.0, 27.5, -1580.0, 2.0, 0, 0, null),
    (1000243, 'man0g1_pa4_burchard',   155, 'PrivateAreaMasterPast', 4, 190.0, 27.5, -1579.0, 2.0, 0, 0, null),
    (1000015, 'man0g1_pa4_tkebbe',     155, 'PrivateAreaMasterPast', 4, 182.0, 27.5, -1581.0, 2.0, 0, 0, null),
    (1000017, 'man0g1_pa4_farrimond',  155, 'PrivateAreaMasterPast', 4, 183.0, 27.5, -1580.0, 2.2, 0, 0, null),
    (1000683, 'man0g1_pa4_cecilia',    155, 'PrivateAreaMasterPast', 4, 184.0, 27.5, -1578.0, 2.4, 0, 0, null),
    (1000682, 'man0g1_pa4_mansel',     155, 'PrivateAreaMasterPast', 4, 185.0, 27.5, -1577.0, 0.0, 0, 1007, null),
    (1000741, 'man0g1_pa4_jijimaya',   155, 'PrivateAreaMasterPast', 4, 180.0, 27.5, -1583.0, 0.0, 0, 1007, null);
