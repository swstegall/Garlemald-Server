-- Man0g1 "Souls Gone Wild" — start the SEQ_065 escort at the White Wolf
-- Gate (Garlemald-Server #41, record-mode round, live 2026-07-06).
--
-- The duty warped the player to (-700,21,-1000) — mid-Central-Shroud, a
-- stone's throw south of the Lifemend Stump — so the "escort" began
-- essentially AT the destination. Retail starts you at the gate and you
-- walk the whole Central Shroud to the stump. The zone-150 side of the
-- Gridania (155) → Central Shroud (150) seam is the box (304..338,
-- -1046..-1066) — server_seamless_zonechange_bounds row 2 — i.e. the
-- White Wolf Gate crossing in zone-150 coordinates, midpoint
-- ~(321, 4, -1056). It is confirmed-walkable (players cross there
-- seamlessly). The warp-in moves there (man0g1.lua startMan0g1Content +
-- SimpleContentMan0g101 WARP_IN) so the player walks WEST to the stump.
--
-- The kids MUST spawn WITH the player: the content roster only streams
-- actors within INSTANCE_STREAM_RADIUS (50) of the warp point, so kids
-- left at the old (-700,-1000) spot would never enter the roster and the
-- escort would stall (onUpdate never sees them). Move bnpc 34/35 beside
-- the new warp-in. The ankle biters (bnpc 36-41) stay near the stump end
-- for now — the recorded walk re-homes them along the real arc (seed/072
-- recipe).
--
-- Idempotent: guarded on the old spawn X so a re-run is a no-op. No
-- schema change.

UPDATE "server_battlenpc_spawn_locations"
SET "positionX" = 318.0, "positionY" = 4.0, "positionZ" = -1058.0, "rotation" = -1.6
WHERE "bnpcId" = 34 AND "positionX" = -702.0;

UPDATE "server_battlenpc_spawn_locations"
SET "positionX" = 324.0, "positionY" = 4.0, "positionZ" = -1058.0, "rotation" = -1.6
WHERE "bnpcId" = 35 AND "positionX" = -699.0;
