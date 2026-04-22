require ("global")
require ("quest")

--[[

Quest Script

Name: 	Bridging the Gap
Code: 	Etc1l1
Id: 	110634
Prereq: Level 10, Any Class

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Toll Puks.
SEQ_001	= 1;  -- Talk to Hihine.

-- Actor Class Ids
ENPC_HIHINE 		= 1000267;
BNPC_TOLL_PUK		= 2100113;

-- Quest Markers
MRKR_TOLLPUK_AREA	= 11063401;
MRKR_HIHINE			= 11063402;

-- Counters
COUNTER_KILLS		= 0;

-- Quest Details
OBJECTIVE_AMOUNT	= 8;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_HIHINE, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_HIHINE);
		quest:SetENpc(BNPC_TOLL_PUK);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_HIHINE, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_HIHINE and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventHihineStart", OBJECTIVE_AMOUNT);
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_HIHINE) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent005_2", OBJECTIVE_AMOUNT);
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_HIHINE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_TOLL_PUK) then
		local counterAmount = quest:GetData():IncCounter(COUNTER_KILLS);
		attentionMessage(player, 50041, 3100116, counterAmount, OBJECTIVE_AMOUNT); -- The <dispName> has been defeated. (X of Y)
        if (counterAmount >= OBJECTIVE_AMOUNT) then
			attentionMessage(player, 25225, quest:GetQuestId()); -- Objectives complete!
			quest:StartSequence(SEQ_001);
		end
	end
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_TOLLPUK_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_HIHINE;
    end
end