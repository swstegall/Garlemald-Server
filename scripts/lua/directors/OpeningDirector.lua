require("global")

function init()
	return "/Director/OpeningDirector";
end

-- Port of the `ioncannon/quest_system` branch of Meteor — OpeningDirector's
-- `onEventStarted` delegates straight to the quest's `onNotice` hook, which
-- is the one that actually fires the opening cutscene + EndEvent. Keeping
-- the cutscene logic inside the quest script (not the director) keeps each
-- quest self-contained and lets the director survive without knowing any
-- per-quest flow.
function onEventStarted(player, actor, eventTrigger, eventName)
	if (player:HasQuest(110001) == true) then
		quest = player:GetQuest(110001);
		quest:OnNotice(player);
	elseif (player:HasQuest(110005) == true) then
		quest = player:GetQuest(110005);
		quest:OnNotice(player);
	elseif (player:HasQuest(110009) == true) then
		quest = player:GetQuest(110009);
		quest:OnNotice(player);
	end
end

function main()
end

function onUpdate()
end

function onTalkEvent(player, npc)

	if (player:HasQuest(110001) == true) then
		man0l0Quest = player:GetQuest(110001);
		
		if (man0l0Quest:GetQuestFlag(MAN0L0_FLAG_MINITUT_DONE1) == true and man0l0Quest:GetQuestFlag(MAN0L0_FLAG_MINITUT_DONE2) == true and man0l0Quest:GetQuestFlag(MAN0L0_FLAG_MINITUT_DONE3) == true) then
			doorNpc = GetWorldManager():GetActorInWorldByUniqueId("exit_door");		
			player:SetEventStatus(doorNpc, "pushDefault", true, 0x2);
			doorNpc:SetQuestGraphic(player, 0x3);
		end
	elseif (player:HasQuest(110005) == true) then	
		man0g0Quest = player:GetQuest(110005);
		if (man0g0Quest:GetQuestFlag(MAN0L0_FLAG_STARTED_TALK_TUT) == true and man0g0Quest:GetQuestFlag(MAN0G0_FLAG_MINITUT_DONE1) == false) then
			papalymo = GetWorldManager():GetActorInWorldByUniqueId("papalymo");	
			papalymo:SetQuestGraphic(player, 0x2);
		elseif (man0g0Quest:GetQuestFlag(MAN0L0_FLAG_STARTED_TALK_TUT) == true and man0g0Quest:GetQuestFlag(MAN0G0_FLAG_MINITUT_DONE1) == true) then			
			yda = GetWorldManager():GetActorInWorldByUniqueId("yda");	
			yda:SetQuestGraphic(player, 0x2);
		end
	elseif (player:HasQuest(110009) == true) then	
		man0u0Quest = player:GetQuest(110009);
		if (man0u0Quest:GetQuestFlag(MAN0U0_FLAG_MINITUT_DONE1) == true and man0u0Quest:GetQuestFlag(MAN0U0_FLAG_MINITUT_DONE2) == true and man0u0Quest:GetQuestFlag(MAN0U0_FLAG_MINITUT_DONE3) == true) then			
			exitTriggerNpc = GetWorldManager():GetActorInWorldByUniqueId("exit_trigger");		
			player:SetEventStatus(exitTriggerNpc, "pushDefault", true, 0x2);
			exitTriggerNpc:SetQuestGraphic(player, 0x2);					
		end
	end

end

function onPushEvent(player, npc)
end

function onCommandEvent(player, command)
end

function onEventUpdate(player, npc)
end

function onCommand(player, command)	
end