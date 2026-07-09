-- Man0g1 SEQ_050/055 dance-practice fixups (Garlemald-Server #41, live
-- 2026-07-06 with retail-reference frames).
--
-- Three fixes:
--   (1) OPYLTYL out of the instanced scenes. He is the ENTRY npc only
--       (SEQ_040/050 public: talk him -> warp into the drill). Inside
--       the drill his onTalk branch RE-WARPS the player into PA1
--       (man0g1.lua SEQ_050 onTalk, "== OPYLTYL" -> DoZoneChange PA1),
--       so brushing him mid-dance resets the drill and blocks the
--       auto-advance to the Zezekuta/Fufucha cutscene ("he bugs it").
--       Retail: only Fufucha + the six kids are on the practice
--       grounds. Seed/078 wrongly spawned Opyltyl in PA1 and PA2 --
--       DELETE both. His public 206 station (base spawn) stays, so the
--       SEQ_040 entry + SEQ_050/055 re-entry recovery still work.
--   (2) UNDERGROUND kids. Seed/078 spread the drill cast to
--       z <= -1510, where the Growery ground rises above the warp
--       anchor's y=12 (navmesh is a stub -- no ground-height query), so
--       Elyn/Nicollaux sank. The player stands fine AT the anchor
--       (-223.792, 12, -1498.369), so keep every kid within a few
--       units of the anchor's z (the proven-flat band z ~ -1494..-1502)
--       and spread horizontally in x instead -- matching the retail
--       left-to-right arc (Ryd, Aunillie, Sansa | player | Powle, Elyn,
--       Nicollaux; ref frame image #10). Still provisional y; a precise
--       pass = stand-and-mark the ground from a live 0x00CA report.
--   (3) SEQ_055 chat kids pulled to the same flat band around their PA2
--       anchor (-231.474, 12, -1500.86) so they don't sink there too.

-- ---- (1) Opyltyl out of the instances --------------------------------

DELETE FROM "server_spawn_locations" WHERE "uniqueId" IN ('man0g1_opyltyl_drill', 'man0g1_opyltyl_chat');

-- ---- (2) SEQ_050 drill kids: retail arc, safe ground ------------------

UPDATE "server_spawn_locations" SET "positionX" = -238.0, "positionY" = 12.0, "positionZ" = -1497.0, "rotation" =  1.40 WHERE "uniqueId" = 'man0g1_ryd_drill';
UPDATE "server_spawn_locations" SET "positionX" = -233.0, "positionY" = 12.0, "positionZ" = -1495.0, "rotation" =  1.10 WHERE "uniqueId" = 'man0g1_aunillie_drill';
UPDATE "server_spawn_locations" SET "positionX" = -228.0, "positionY" = 12.0, "positionZ" = -1494.0, "rotation" =  0.60 WHERE "uniqueId" = 'man0g1_sansa_drill';
UPDATE "server_spawn_locations" SET "positionX" = -219.0, "positionY" = 12.0, "positionZ" = -1494.0, "rotation" = -0.60 WHERE "uniqueId" = 'man0g1_powle_drill';
UPDATE "server_spawn_locations" SET "positionX" = -214.0, "positionY" = 12.0, "positionZ" = -1495.0, "rotation" = -1.10 WHERE "uniqueId" = 'man0g1_elyn_drill';
UPDATE "server_spawn_locations" SET "positionX" = -210.0, "positionY" = 12.0, "positionZ" = -1497.0, "rotation" = -1.40 WHERE "uniqueId" = 'man0g1_nicollaux_drill';
-- Fufucha the instructor stays, just beside the player at the anchor.
UPDATE "server_spawn_locations" SET "positionX" = -224.0, "positionY" = 12.0, "positionZ" = -1502.0, "rotation" =  0.00 WHERE "uniqueId" = 'man0g1_fufucha_drill';

-- ---- (3) SEQ_055 chat kids: tight cluster on flat ground -------------

UPDATE "server_spawn_locations" SET "positionX" = -233.0, "positionY" = 12.0, "positionZ" = -1498.0, "rotation" =  0.30 WHERE "uniqueId" = 'man0g1_fufucha_chat';
UPDATE "server_spawn_locations" SET "positionX" = -237.0, "positionY" = 12.0, "positionZ" = -1500.0, "rotation" =  0.70 WHERE "uniqueId" = 'man0g1_aunillie_chat';
UPDATE "server_spawn_locations" SET "positionX" = -239.0, "positionY" = 12.0, "positionZ" = -1502.0, "rotation" =  1.20 WHERE "uniqueId" = 'man0g1_nicollaux_chat';
UPDATE "server_spawn_locations" SET "positionX" = -235.0, "positionY" = 12.0, "positionZ" = -1503.0, "rotation" =  0.30 WHERE "uniqueId" = 'man0g1_sansa_chat';
UPDATE "server_spawn_locations" SET "positionX" = -230.0, "positionY" = 12.0, "positionZ" = -1503.0, "rotation" = -0.20 WHERE "uniqueId" = 'man0g1_powle_chat';
UPDATE "server_spawn_locations" SET "positionX" = -227.0, "positionY" = 12.0, "positionZ" = -1501.0, "rotation" = -0.60 WHERE "uniqueId" = 'man0g1_ryd_chat';
UPDATE "server_spawn_locations" SET "positionX" = -231.0, "positionY" = 12.0, "positionZ" = -1502.0, "rotation" =  0.10 WHERE "uniqueId" = 'man0g1_elyn_chat';
