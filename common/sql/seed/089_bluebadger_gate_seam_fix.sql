-- Blue Badger Gate (Gridania 155 -> Central Shroud 150) seamless seam fix
-- (Garlemald-Server #41 tail; world-infra, not quest-specific).
--
-- man0g1 SEQ_005 sends the player east through the Blue Badger Gate to Camp
-- Bentbranch, but the gate is a solid client-collision barrier and garlemald
-- has NO gate-open mechanism — the only way through is the 155<->150 seamless
-- swap (the client drops the 155 gate geometry on the zone change). Seed/029
-- boundary #2's transition box sat at x 304-338, z -1066..-1046 — ~140 units
-- off the REAL gate, so the seam never fired and the player hit the wall.
--
-- Real gate location = the retail Blue Badger Gate guard NPCs (pcap dump
-- captures/gridania-retail-npc-spawns): Bertennant 1000071 (191.07,-1147.25),
-- Mathye 1001432 (179.56,-1144.66), Lonsygg 1000951 (162.79,-1153.32), Ulta
-- 1001433 (192.07,-1159.97) — i.e. x~163..192, z~-1145..-1160. The player
-- approaches from the Carline Canopy (west, x~58) facing +x (east) into the
-- gate; the Shroud is east (+x). Place the 150 trigger box just WEST of the
-- gate collision (x >= 182) so crossing east swaps to 150 BEFORE the wall,
-- with 155 on the town side (x <= 181). One shared coordinate space (155/206/
-- 150), so the swap keeps the player's x/z.
--
-- New seed (029 already applied). LIVE TEST: does the swap clear the gate
-- collision? If yes -> real fix; if still blocked -> needs a gate-open packet.

UPDATE "server_seamless_zonechange_bounds" SET
    "zone1_boundingbox_x1" = 140,  "zone1_boundingbox_y1" = -1168,
    "zone1_boundingbox_x2" = 181,  "zone1_boundingbox_y2" = -1138,
    "zone2_boundingbox_x1" = 182,  "zone2_boundingbox_y1" = -1168,
    "zone2_boundingbox_x2" = 220,  "zone2_boundingbox_y2" = -1138,
    "merge_boundingbox_x1" = 181,  "merge_boundingbox_y1" = -1168,
    "merge_boundingbox_x2" = 182,  "merge_boundingbox_y2" = -1138
WHERE "regionId" = 103 AND "zoneId1" = 155 AND "zoneId2" = 150;
