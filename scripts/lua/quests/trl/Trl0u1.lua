require ("global")

--[[

Quest Script

Name:	Getting Started (Momodi)
Code:	Trl0u1
Id:		110142

Enables the "Getting Started" option on Momodi.
* NOTE: This quest is active for all players at all times.
]]

function onTalk(player, quest, npc, eventName)
	local choice = callClientFunction(player, "delegateEvent", player, quest, "processEventMomodiStart");	
	
	if (choice == 1) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent235");
	elseif (choice == 2) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent240");
	end
	
	player:EndEvent();
end

function IsQuestENPC(player, quest, npc)
	return npc:GetActorClassId() == 1000841;
end