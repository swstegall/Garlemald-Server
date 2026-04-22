require ("global")

--[[

Quest Script

Name:	Getting Started (Baderon)
Code:	Trl0l1
Id:		110140

Enables the "Getting Started" option on Baderon.
* NOTE: This quest is active for all players at all times.
]]

function onTalk(player, quest, npc, eventName)
	local choice = callClientFunction(player, "delegateEvent", player, quest, "processEventBaderonStart");	
	
	if (choice == 1) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent640");
	elseif (choice == 2) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent650");
	end
	
	player:EndEvent();
end

function IsQuestENPC(player, quest, npc)
	return npc:GetActorClassId() == 1000137;
end