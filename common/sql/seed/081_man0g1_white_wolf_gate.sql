-- Man0g1 — pin the White Wolf Gate escort trigger to the real gate
-- (Garlemald-Server #41, live 2026-07-06 stand-and-mark).
--
-- The player's exact position at the true White Wolf Gate (decoded from
-- the previous session's last 0x00CA position packet, peer 64785):
--     x = -194.73, y = 3.54, z = -1021.33, rot = -1.642
-- The provisional GATE_TRIGGER spot (seed/074 -235/-1455, later 206) was
-- at the Greatloam Growery -- out of bounds and ~460u south of the gate,
-- so the duty could never start there.
--
-- Zone is ambiguous from the data: z=-1021 sits well NORTH of the
-- 155<->206 town seam (z ~ -1217..-1279, seed/029 row 1) and at Roost
-- ground level (y~3.5) -- pointing to 155 -- but the 07:08 log window has
-- rolled off, so it can't be confirmed 155 vs 206. HEDGE: spawn the
-- trigger (class 1090202, push circle radius 25 from seed/074) at the
-- gate coords in BOTH zones. A single XZ exists in exactly one zone, so
-- the other copy is inert (the player is never in that zone at that
-- spot); whichever half the gate actually is, the trigger is present and
-- arms via the cross-zone re-establish (665ee92). No double-fire: the
-- two copies are in different zones, so any given zone has exactly one.

-- Move the existing trigger to the gate (keep it in 206 as one hedge).
UPDATE "server_spawn_locations"
SET "zoneId" = 206, "positionX" = -194.73, "positionY" = 3.5, "positionZ" = -1021.33, "rotation" = -1.642
WHERE "uniqueId" = 'man0g1_gate_trig';

-- The 155 hedge copy at the same gate coords.
INSERT OR IGNORE INTO "server_spawn_locations"
    ("actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    (1090202, 'man0g1_gate_trig_n155', 155, '', 0, -194.73, 3.5, -1021.33, -1.642, 0, 0, null);
