require ("global")
require ("quest")

--[[

Quest Script

Name: 	Monster of Maw Most Massive
Code: 	Etc3u9
Id: 	110734
Prereq: Level 45, Any DoW/DoM

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill ?? ?? ??.
SEQ_001	= 1;  -- Talk to Dural Tharal.

-- Actor Class Ids
ENPC_DURAL_THARAL 	= 1002101;
BNPC_MUSK_ROSELING	= 2102717;

-- Quest Markers
MRKR_ROSELING_AREA	= 11063801;
MRKR_DURAL_THARAL	= 11063802;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000149;
OBJECTIVE_AMOUNT	= 8;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_DURAL_THARAL, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_DURAL_THARAL);
		quest:SetENpc(BNPC_MUSK_ROSELING);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_DURAL_THARAL, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_DURAL_THARAL and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventStart", 0, OBJECTIVE_AMOUNT);
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_DURAL_THARAL) then
            callClientFunction(player, "delegateEvent", player, quest, "processEventFree", 0, OBJECTIVE_AMOUNT);
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_DURAL_THARAL) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventClear");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (bnpc == BNPC_MUSK_ROSELING) then
		local counterAmount = quest:GetData():IncCounter(COUNTER_QUESTITEM);
		attentionMessage(player, 25226, OBJECTIVE_ITEMID, 1, counterAmount, OBJECTIVE_AMOUNT); -- You obtain <item> (X of Y)
        if (counterAmount >= OBJECTIVE_AMOUNT) then
			attentionMessage(player, 25225, quest:GetQuestId()); -- Objectives complete!
			quest:StartSequence(SEQ_001);
		end
	end
end

function getJournalInformation(player, quest)
	return quest:GetData():GetCounter(COUNTER_QUESTITEM);
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_ROSELING_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_DURAL_THARAL;
    end
end