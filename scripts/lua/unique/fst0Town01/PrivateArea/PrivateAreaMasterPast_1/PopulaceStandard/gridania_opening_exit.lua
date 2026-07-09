require ("global")
require ("quests/man/man0g0")

function onSpawn(player, npc)	
	npc:SetQuestGraphic(player, 0x3);	
end

function onEventStarted(player, npc)
	man0g1Quest = GetStaticActor("Man0g1");
	callClientFunction(player, "delegateEvent", player, man0g1Quest, "processEvent100");
	player:ReplaceQuest(110005, 110006);
	player:SendGameMessage(GetStaticActor("Man0g1"), 353, 0x20);
	player:SendGameMessage(GetStaticActor("Man0g1"), 354, 0x20);
	-- Close the handoff event BEFORE the warp (DoZoneChange ships the zone-in
	-- replay inline; a trailing EndEvent lands mid-reload). Mirrors the proven
	-- man0g1.lua:479-506 exit ordering.
	player:EndEvent();

	-- HARDEN the PA/1 -> PA/2 flip. This warp is a same-zone (155->155) wipe+
	-- reload; un-hardened it does not reliably tear down the client's
	-- PrivateAreaMasterPast/1 view, so the closed Blue Badger Gate prop
	-- (DoorStandard 5900004 @ x~185, PA/1-only) lingers on the client and walls
	-- the player at man0g1 SEQ_005 -- even though the server state is already
	-- correct (a fresh login loads public 155 with the gate open). The
	-- login-director + noticeEvent kick is the proven same-map-reload
	-- completion pattern (man0g1.lua:485-498; AfterQuestWarpDirector routes
	-- 110006 back to onNotice), forcing the reload to complete so the flip's
	-- DeleteAllActors actually clears the gate. (Garlemald-Server #41.)
	local director = GetWorldManager():GetArea(155):CreateDirector("AfterQuestWarpDirector", false);
	director:StartDirector(true);
	player:AddDirector(director);
	player:SetLoginDirector(director);
	player:KickEvent(director, "noticeEvent", true);
	GetWorldManager():DoZoneChange(player, 155, "PrivateAreaMasterPast", 2, 15, 67.034, 4, -1205.6497, -1.074);
end