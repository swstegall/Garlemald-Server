-- Man0g1 "Souls Gone Wild" — anchor the SEQ_065 escort warp-in to the
-- EXACT White Wolf Gate coordinates where the "Duty calls" dialog fires
-- (Garlemald-Server #41, record-mode round 2, live 2026-07-06).
--
-- Round 1 (seed/083) used the zone-150 side of a 155<->150 seam
-- (~321,4,-1056), but that seam is the BLUE BADGER GATE — the player
-- loaded onto its structure, slightly clipped, at the wrong gate. Simpler
-- and correct: the escort instance is a copy of zone 150, and the whole
-- Gridania region (155/206/150) is one seamless coordinate space, so the
-- gate-trigger position the player stood at when the dialog fired IS a
-- valid zone-150 point. Use it verbatim: (-194.73, 3.54, -1021.33), rot
-- -1.642 — the stand-and-marked White Wolf Gate (seed/081, decoded from
-- the player's 0x00CA at the gate).
--
-- The kids spawn beside the player there (else they never stream into the
-- content roster, 50u radius). Biters stay near the stump end until the
-- recorded walk re-homes them.
--
-- Idempotent: guarded on the seed/083 X so a re-run is a no-op. No schema
-- change.

UPDATE "server_battlenpc_spawn_locations"
SET "positionX" = -197.7, "positionY" = 3.54, "positionZ" = -1023.3, "rotation" = -1.642
WHERE "bnpcId" = 34 AND "positionX" = 318.0;

UPDATE "server_battlenpc_spawn_locations"
SET "positionX" = -191.7, "positionY" = 3.54, "positionZ" = -1023.3, "rotation" = -1.642
WHERE "bnpcId" = 35 AND "positionX" = 324.0;
