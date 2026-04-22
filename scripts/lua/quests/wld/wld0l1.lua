require ("global")
require ("quest")

--[[

Quest Script

Name: 	Trading Tongueflaps 
Code: 	Wld0l1
Id: 	110771
Prereq: Level 5, Any Class

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Talk to Ryssfloh.
SEQ_001	= 1;  -- Talk to Sweetnix Rosycheeks.

-- Actor Class Ids
SWEETNIX 		= 1001573;
RYSSFLOH	 	= 1000359;

-- Quest Markers
MRKR_RYSSFLOH	= 11110001;
MRKR_SWEETNIX	= 11110002;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(SWEETNIX, QFLAG_TALK);
	end

	if (sequence == SEQ_000) then
        quest:SetENpc(SWEETNIX);
		quest:SetENpc(RYSSFLOH, QFLAG_TALK);
	elseif (sequence == SEQ_001) then	
		quest:SetENpc(RYSSFLOH);
		quest:SetENpc(SWEETNIX, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == SWEETNIX and not player:HasQuest(quest)) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventSweetnixStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;
	end
	
	-- Quest Progress
	if (seq == SEQ_000) then
        if (npcClassId == SWEETNIX) then
            callClientFunction(player, "delegateEvent", player, quest, "followEvent005");
		elseif (npcClassId == RYSSFLOH) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
			quest:StartSequence(SEQ_001);
		end
	elseif (seq == SEQ_001) then
		--Quest Complete
		if (npcClassId == SWEETNIX) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		elseif (npcClassId == RYSSFLOH) then
			callClientFunction(player, "delegateEvent", player, quest, "followEvent015");
			quest:StartSequence(SEQ_001);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_RYSSFLOH;
    elseif (sequence == SEQ_001) then
        return MRKR_SWEETNIX;
    end
end