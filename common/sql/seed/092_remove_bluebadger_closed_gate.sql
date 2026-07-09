-- Remove the man0g0 closed Blue Badger Gate prop (Garlemald-Server #41).
--
-- The closed gate is a SERVER-SPAWNED MapObj DoorStandard actor:
--   server_spawn_locations id 871 'closed_gridania_gate', actorClassId 5900004
--   (/Chara/Npc/MapObj/DoorStandard), at (185,-1,-1154), spawned ONLY in zone-155
--   PrivateAreaMasterPast level 1 -- the man0g0 "Sundered Skies" opening instance.
--   server_eventnpc_mapobj row 871 (layoutId 321, instanceId 2937) makes it emit as a
--   0x00D8 SetActorBGProperties, hanging a MapObjectForChara door mesh + collision off it.
--
-- The bug: that BG mesh attaches to the client during man0g0 (the player is in PA/1) and
-- is ORPHANED across man0g1's same-zone (155->155) soft reloads -- the client deletes the
-- actor but the attached BG mesh only tears down on a FULL scene teardown (a fresh login
-- or a real zone change), NOT on a same-zone reload. So the player reaches man0g1's public
-- New Gridania (SEQ_005) still carrying the ghost gate and is walled at the Blue Badger
-- Gate, blocking the seamless 155->150 crossing to Camp Bentbranch. Verified live
-- (2026-07-09): a fresh login into clean public 155 renders the gate OPEN and crosses to
-- Central Shroud seamlessly -- confirming the wall is solely this leaked opening-instance
-- prop, and that public New Gridania is the correct open-gate variant.
--
-- Fix (seamless, no hard warp/seam): never attach the prop. man0g0 stays contained by its
-- existing BLOCKER1 push (actorClass 1099047); the gate simply appears open during the
-- opening tutorial. No quest logic references spawn 871 (grep-verified). Drop its mapobj
-- row too so no orphaned layout binding remains.
--
-- Idempotent: DELETE of an absent row is a no-op (fresh DBs never carry it past this seed;
-- migrated DBs lose it here).

DELETE FROM "server_spawn_locations" WHERE "id" = 871 AND "uniqueId" = 'closed_gridania_gate';
DELETE FROM "server_eventnpc_mapobj" WHERE "id" = 871;
