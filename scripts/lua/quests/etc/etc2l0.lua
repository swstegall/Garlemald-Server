require ("global")
require ("quest")

--[[

Quest Script

Name: 	Fishing for Answers
Code: 	Etc2l0
Id: 	110643
Prereq: Level 25, Any DoW/DoM

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Giant Crab.
SEQ_001	= 1;  -- Talk to Robairlain.

-- Actor Class Ids
ENPC_ROBAIRLAIN 	= 1000050;
BNPC_GIANT_CRAB		= 2107601;

-- Quest Markers
MRKR_CRAB_AREA		= 11064301;
MRKR_ROBAIRLAIN		= 11064302;

-- Counters
COUNTER_KILLS		= 0;

-- Quest Details
OBJECTIVE_AMOUNT	= 5;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_ROBAIRLAIN, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_ROBAIRLAIN);
		quest:SetENpc(BNPC_GIANT_CRAB);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_ROBAIRLAIN, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_ROBAIRLAIN and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventEadbertStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_ROBAIRLAIN) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent000");
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_ROBAIRLAIN) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_GIANT_CRAB) then
		local counterAmount = quest:GetData():IncCounter(COUNTER_KILLS);
		attentionMessage(player, 50041, 3107601, counterAmount, OBJECTIVE_AMOUNT); -- The <dispName> has been defeated. (X of Y)
        if (counterAmount >= OBJECTIVE_AMOUNT) then
			attentionMessage(player, 25225, quest:GetQuestId()); -- Objectives complete!
			quest:StartSequence(SEQ_001);
		end
	end
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_CRAB_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_ROBAIRLAIN;
    end
end