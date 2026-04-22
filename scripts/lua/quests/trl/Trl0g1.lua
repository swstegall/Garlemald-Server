require ("global")

--[[

Quest Script

Name: 	Getting Started (Mother Miounne)
Code: 	Trl0g1
Id: 	110141

Enables the "Getting Started" option on Miounne.
* NOTE: This quest is active for all players at all times.
]]

function onTalk(player, quest, npc, eventName)
	local choice = callClientFunction(player, "delegateEvent", player, quest, "processEventMiounneStart");	
	
	if (choice == 1) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent225");
	elseif (choice == 2) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent230");
	end
	
	player:EndEvent();
end

function IsQuestENPC(player, quest, npc)
	return npc:GetActorClassId() == 1000230;
end