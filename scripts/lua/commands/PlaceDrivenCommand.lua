--[[

PlaceDrivenCommand Script

Notes: 


--]]
require("global")

function onEventStarted(player, actor, triggerName, pushCommand, unk1, unk2, unk3, ownerActorId, unk4, unk5, unk6, unk7)
	
	actor = player:GetActorInInstance(ownerActorId);
	
	if (actor != nil) then
		if (actor:GetActorClassId() == 1200052) then
			player:KickEvent(actor, "commandJudgeMode", "commandJudgeMode");
		else
			printf("TEST");
			player:KickEvent(actor, "pushCommand", "pushCommand");
		end
	else
		player:EndEvent();
	end
	
end