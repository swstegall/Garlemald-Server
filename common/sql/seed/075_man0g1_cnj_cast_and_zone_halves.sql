-- Man0g1 SEQ_015 — the Conjurers' Guild arc cast + Gridania zone-half
-- corrections (Garlemald-Server #41, live 2026-07-06 follow-up).
--
-- Gridania is TWO seamless halves sharing one coordinate space (bound
-- seed/029 row 1, region 103): zone 155 fst0Town01 = the NORTH half
-- (Carline Canopy / Roost cluster + ALL 17 quest private-area scene
-- rows upstream) and zone 206 fst0Town01a = the SOUTH half (the guild
-- district: Hereward -1459, Greatloam Growery -1477, the aetheryte
-- plaza — 154 populace rows). The boundary box sits at z ≈ -1261..-1279,
-- so anything south of z ≈ -1270 belongs in 206.
--
-- Live findings this seed fixes:
--   (1) The ENTIRE SEQ_015 CNJ cast has ZERO spawn rows in any
--       upstream — Soileine (the QFLAG_TALK gate) and the Stillglade
--       Echo scene cast simply never existed; the "guild loads no new
--       NPCs" report is literal. Net-new rows below (scene positions
--       PROVISIONAL — clustered on the quest's own warp anchor, same
--       tuning caveat as seed/074).
--   (2) seed/074 placed the CNJ/gate/BTN triggers in 155, but their
--       stations are all south of the boundary — re-homed to 206
--       (BTN's upstream row was 206 all along; the 074 "translation"
--       was wrong).

-- ---- (1) Trigger zone-half corrections (074 rows) --------------------

UPDATE "server_spawn_locations" SET "zoneId" = 206
WHERE "uniqueId" IN ('man0g1_cnj_trig', 'man0g1_gate_trig', 'man0g1_btn_trig')
  AND "zoneId" = 155;

-- ---- (2) Soileine — the Fane door gate NPC (public, south half) ------

INSERT OR IGNORE INTO "server_spawn_locations"
    ("actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    -- At the Stillglade Fane entrance, beside the CNJ push trigger
    -- (provisional — the quest's PA warp anchor mirrored public).
    (1000234, 'man0g1_soileine', 206, '', 0, -350.0, 6.25, -1694.0, 0.77, 0, 1015, null);

-- ---- (3) The Stillglade Echo scene cast (155 PrivateAreaMasterPast 0) --
-- The CNJ_TRIG push warps into PrivateAreaMasterPast level 0 at
-- (-353.05, 6.25, -1697.39) — the area seed/074 registered. Scene cast
-- per man0g1.lua SEQ_015 (O-App-Pesi lore scene, retail Part 2 of the
-- playthrough): O-App-Pesi front and center, Yda/Papalymo brought in by
-- Swethyna, guild vets around the chamber. All positions PROVISIONAL
-- (clustered offsets on the warp anchor; in-client tuning round).
INSERT OR IGNORE INTO "server_spawn_locations"
    ("actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    (1000033, 'man0g1_oapppesi_echo',  155, 'PrivateAreaMasterPast', 0, -359.0, 6.25, -1703.0,  0.60, 0, 1015, null),
    (1000009, 'man0g1_yda_echo',       155, 'PrivateAreaMasterPast', 0, -356.5, 6.25, -1699.5, -2.20, 0, 1007, null),
    (1000010, 'man0g1_papalymo_echo',  155, 'PrivateAreaMasterPast', 0, -354.5, 6.25, -1701.0, -2.60, 0, 1002, null),
    (1000680, 'man0g1_swethyna_echo',  155, 'PrivateAreaMasterPast', 0, -351.0, 6.25, -1700.5,  2.40, 0, 1015, null),
    (1000372, 'man0g1_ingram_echo',    155, 'PrivateAreaMasterPast', 0, -348.0, 6.25, -1705.0,  1.20, 0, 1015, null),
    (1000460, 'man0g1_hetzkin_echo',   155, 'PrivateAreaMasterPast', 0, -346.5, 6.25, -1701.5,  1.60, 0, 1015, null),
    (1000513, 'man0g1_gugula_echo',    155, 'PrivateAreaMasterPast', 0, -349.5, 6.25, -1709.0,  0.90, 0, 1015, null),
    (1000737, 'man0g1_biddy_echo',     155, 'PrivateAreaMasterPast', 0, -357.0, 6.25, -1707.5,  0.30, 0, 1015, null),
    (1000956, 'man0g1_challinie_echo', 155, 'PrivateAreaMasterPast', 0, -344.0, 6.25, -1697.0,  2.00, 0, 1015, null);
