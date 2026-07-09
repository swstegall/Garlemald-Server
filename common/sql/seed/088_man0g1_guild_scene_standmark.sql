-- Man0g1 "Souls Gone Wild" — SEQ_090/095 guild-scene cast: exact stand-and-mark
-- placements (Garlemald-Server #41).
--
-- seed/087's provisional coords buried the cast: the 155 PrivateAreaMasterPast
-- floor sits higher than the 27.5 warp-in y AND slopes (y ranges 27.5 near the
-- south wall to 29.5 by the x~208 group). These are the player's in-client
-- stand-and-mark readings (via the temp position log in handle_update_position).
-- Applied to BOTH PA level 3 (SEQ_090 Burchard beat) and level 4 (SEQ_095 Nuala
-- beat) — the two beats share the guild room + floor and these seven appear in
-- both casts. Position + rotation only; poses (standing / collapsed animationId)
-- stay as seed/087 set them.
--
-- New seed: 087 is already applied to the live DB, so re-editing it would not
-- re-run — UPDATE via a fresh migration. Still-provisional (no stand-and-mark
-- yet): the PA3-only Langloisiert / Turstin / Helbhanth / Pasdevillet.

UPDATE "server_spawn_locations" SET "positionX"=195.6979522705078, "positionY"=27.900455474853516, "positionZ"=-1577.485107421875, "rotation"=-2.7279040813446045
    WHERE "uniqueId" IN ('man0g1_pa3_nuala', 'man0g1_pa4_nuala');
UPDATE "server_spawn_locations" SET "positionX"=194.451416015625, "positionY"=27.900455474853516, "positionZ"=-1577.028076171875, "rotation"=3.1395442485809326
    WHERE "uniqueId" IN ('man0g1_pa3_burchard', 'man0g1_pa4_burchard');
UPDATE "server_spawn_locations" SET "positionX"=183.7240753173828, "positionY"=27.50001335144043, "positionZ"=-1594.3448486328125, "rotation"=0.812190055847168
    WHERE "uniqueId" IN ('man0g1_pa3_jijimaya', 'man0g1_pa4_jijimaya');
UPDATE "server_spawn_locations" SET "positionX"=208.4712371826172, "positionY"=29.5, "positionZ"=-1577.7032470703125, "rotation"=3.1318297386169434
    WHERE "uniqueId" IN ('man0g1_pa3_mansel', 'man0g1_pa4_mansel');
UPDATE "server_spawn_locations" SET "positionX"=208.73329162597656, "positionY"=29.5, "positionZ"=-1578.9500732421875, "rotation"=-0.005760669708251953
    WHERE "uniqueId" IN ('man0g1_pa3_cecilia', 'man0g1_pa4_cecilia');
UPDATE "server_spawn_locations" SET "positionX"=208.0318145751953, "positionY"=29.5, "positionZ"=-1582.166259765625, "rotation"=-0.013352394104003906
    WHERE "uniqueId" IN ('man0g1_pa3_tkebbe', 'man0g1_pa4_tkebbe');
UPDATE "server_spawn_locations" SET "positionX"=208.54454040527344, "positionY"=29.5, "positionZ"=-1581.1256103515625, "rotation"=-0.569556713104248
    WHERE "uniqueId" IN ('man0g1_pa3_farrimond', 'man0g1_pa4_farrimond');
