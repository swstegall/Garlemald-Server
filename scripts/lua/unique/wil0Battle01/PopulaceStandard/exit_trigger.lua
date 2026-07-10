require ("global")
require ("quests/man/man0u0")

function onSpawn(player, npc)

	man0u0Quest = player:GetQuest("Man0u0");

	if (man0u0Quest ~= nil) then
		player:SetEventStatus(npc, "pushDefault", true, 0x2);
		-- (was `man0U0Quest` on the DONE2 read — a nil-index typo)
		if (man0u0Quest ~= nil and man0u0Quest:GetQuestFlag(MAN0U0_FLAG_MINITUT_DONE1) == true and man0u0Quest:GetQuestFlag(MAN0U0_FLAG_MINITUT_DONE2) == true and man0u0Quest:GetQuestFlag(MAN0U0_FLAG_MINITUT_DONE3) == true) then
			npc:SetQuestGraphic(player, 0x3);
		else
			npc:SetQuestGraphic(player, 0x0);
		end
	end

end

function onEventStarted(player, npc, triggerName)
	-- player:GetQuest, NOT GetStaticActor: LuaStaticActor exposes no
	-- GetQuestFlag (the "attempt to call a nil value" error on every
	-- post-claim push of this trigger — live 2026-07-10 01:36; the
	-- Limsa twin exit_door.lua uses GetQuest). This legacy arm only
	-- fires when the quest no longer claims the push (SEQ_005+), so
	-- nil-guard and bail rather than re-run the content handoff.
	man0u0Quest = player:GetQuest("Man0u0");
	-- Only meaningful at SEQ_000 (where the quest's own onPush claims
	-- the armed trigger first anyway). Post-advance strays — like the
	-- duplicate client push right after the content warp — must NOT
	-- re-create the tutorial content (the flags stay 0xF forever, so
	-- without the sequence gate this arm would spawn a second content
	-- area mid-tutorial). Close and bail.
	if (man0u0Quest == nil or man0u0Quest:GetSequence() ~= SEQ_000) then
		player:EndEvent();
		return;
	end

	if (man0u0Quest:GetQuestFlag(MAN0U0_FLAG_MINITUT_DONE1) == true and man0u0Quest:GetQuestFlag(MAN0U0_FLAG_MINITUT_DONE2) == true and man0u0Quest:GetQuestFlag(MAN0U0_FLAG_MINITUT_DONE3) == true) then
		player:EndEvent();
		
		contentArea = player:GetZone():CreateContentArea(player, "/Area/PrivateArea/Content/PrivateAreaMasterSimpleContent", "man0u01", "SimpleContent30079", "Quest/QuestDirectorMan0u001");
		
		if (contentArea == nil) then
			player:EndEvent();
			return;
		end
		
		director = contentArea:GetContentDirector();		
		player:AddDirector(director);		
		director:StartDirector(false);
		
		player:KickEvent(director, "noticeEvent", true);
		player:SetLoginDirector(director);		
					
		GetWorldManager():DoZoneChangeContent(player, contentArea, -24.34, 192, 34.22, 0.78, 16);
	end
	
end