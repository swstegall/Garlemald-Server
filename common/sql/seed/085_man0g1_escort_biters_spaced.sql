-- Man0g1 "Souls Gone Wild" — space the SEQ_065 ankle biters evenly along
-- the recorded gate→stump route (Garlemald-Server #41, live 2026-07-06).
--
-- The player's recorded walk (SimpleContentMan0g101 TRAIL, a 1004-unit
-- arc from the White Wolf Gate to the Lifemend Stump) is sampled at
-- 1/7..6/7 of its arc length; the six biters (bnpc 36-41) move onto those
-- points at their real recorded ground Y, so each ambush triggers as the
-- player reaches that stretch (seed/072 one-per-point recipe). The kids
-- follow the player and never fight — the biters engage the player.
--
-- Idempotent: guarded on the seed/074 spawn X so a re-run is a no-op. No
-- schema change.

UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -183.06, "positionY" = 3.25,  "positionZ" = -884.72  WHERE "bnpcId" = 36 AND "positionX" = -715.0;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -255.57, "positionY" = 4.12,  "positionZ" = -832.77  WHERE "bnpcId" = 37 AND "positionX" = -724.0;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -376.15, "positionY" = 4.41,  "positionZ" = -886.16  WHERE "bnpcId" = 38 AND "positionX" = -733.0;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -507.36, "positionY" = 6.92,  "positionZ" = -902.53  WHERE "bnpcId" = 39 AND "positionX" = -741.0;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -619.96, "positionY" = 3.60,  "positionZ" = -954.60  WHERE "bnpcId" = 40 AND "positionX" = -747.0;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -634.94, "positionY" = 21.94, "positionZ" = -1070.57 WHERE "bnpcId" = 41 AND "positionX" = -752.0;
