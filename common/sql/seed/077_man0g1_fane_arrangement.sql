-- Man0g1 — Stillglade Fane arrangement + pose corrections from the
-- user's retail video references (Garlemald-Server #41, 2026-07-06).
--
-- Reference frames:
--   (a) Public Fane interior: Soileine and Hetzkin stand together AT
--       THE RECEPTION COUNTER inside the chamber (alongside a third
--       NPC "Foforyo" whose actor class is not yet mapped in our data
--       — no display-name→class table server-side; left out).
--   (b) Post-cutscene Echo scene: Yda + O-App-Pesi + Swethyna stand as
--       a TIGHT cluster directly in front of the player; the guild
--       vets (Hetzkin, Soileine, Gugula, Biddy, "Marcelle"-unmapped)
--       are spread along the raised balcony ring near the doors, NOT
--       on the chamber floor; Concessa (unmapped) on the west path.
--   (c) Yda rendered LYING DOWN in our scene — seed/075 copied her
--       town animationId 1007, which is her man0g0
--       collapsed-in-the-forest pose pack. Scene rows get 0 (default
--       standing idle).
--
-- All coordinates remain provisional offsets around the scene warp
-- anchor (-353.05, 6.25, -1697.39); the arrangement now matches the
-- retail frames in structure (tight trio ~7u ahead of the warp-in,
-- vets pushed to the ring at radius ~15u, counter pair inside the
-- public chamber).

-- ---- (1) Pose fixes ---------------------------------------------------

UPDATE "server_spawn_locations" SET "animationId" = 0
WHERE "uniqueId" IN ('man0g1_yda_echo', 'man0g1_papalymo_echo');

-- ---- (2) Echo scene re-arrangement ------------------------------------

-- The trio, tight cluster in front of the player (retail frame b).
UPDATE "server_spawn_locations" SET "positionX" = -355.0, "positionY" = 6.25, "positionZ" = -1705.0, "rotation" = 0.60
WHERE "uniqueId" = 'man0g1_oapppesi_echo';
UPDATE "server_spawn_locations" SET "positionX" = -356.6, "positionY" = 6.25, "positionZ" = -1703.6, "rotation" = 0.90
WHERE "uniqueId" = 'man0g1_yda_echo';
UPDATE "server_spawn_locations" SET "positionX" = -353.4, "positionY" = 6.25, "positionZ" = -1703.6, "rotation" = 0.30
WHERE "uniqueId" = 'man0g1_swethyna_echo';
UPDATE "server_spawn_locations" SET "positionX" = -358.2, "positionY" = 6.25, "positionZ" = -1702.4, "rotation" = 1.10
WHERE "uniqueId" = 'man0g1_papalymo_echo';

-- Vets out to the balcony ring near the doors.
UPDATE "server_spawn_locations" SET "positionX" = -366.0, "positionY" = 7.5, "positionZ" = -1710.0, "rotation" = 0.80
WHERE "uniqueId" = 'man0g1_hetzkin_echo';
UPDATE "server_spawn_locations" SET "positionX" = -340.5, "positionY" = 7.5, "positionZ" = -1711.0, "rotation" = -0.90
WHERE "uniqueId" = 'man0g1_gugula_echo';
UPDATE "server_spawn_locations" SET "positionX" = -337.5, "positionY" = 7.5, "positionZ" = -1703.0, "rotation" = -1.40
WHERE "uniqueId" = 'man0g1_biddy_echo';
UPDATE "server_spawn_locations" SET "positionX" = -368.5, "positionY" = 7.5, "positionZ" = -1702.0, "rotation" = 1.40
WHERE "uniqueId" = 'man0g1_ingram_echo';
UPDATE "server_spawn_locations" SET "positionX" = -336.5, "positionY" = 7.5, "positionZ" = -1693.5, "rotation" = -1.80
WHERE "uniqueId" = 'man0g1_challinie_echo';

-- Soileine's scene copy on the balcony (visible in both post-cutscene
-- retail frames alongside the vets).
INSERT OR IGNORE INTO "server_spawn_locations"
    ("actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    (1000234, 'man0g1_soileine_echo', 155, 'PrivateAreaMasterPast', 0, -363.5, 7.5, -1694.5, 1.80, 0, 1015, null);

-- ---- (3) Public Fane interior: the counter pair -----------------------

-- Soileine moves from the door to the reception counter (retail frame
-- a), Hetzkin joins her (net-new public row — he previously existed
-- only in the Echo scene).
UPDATE "server_spawn_locations" SET "positionX" = -354.0, "positionY" = 6.25, "positionZ" = -1702.5, "rotation" = -0.40
WHERE "uniqueId" = 'man0g1_soileine';
INSERT OR IGNORE INTO "server_spawn_locations"
    ("actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    (1000460, 'man0g1_hetzkin_fane', 206, '', 0, -351.5, 6.25, -1703.5, -0.70, 0, 1015, null);
