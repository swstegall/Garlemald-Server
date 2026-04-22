require ("global")
require ("quest")

--[[

Quest Script

Name: 	Freedom Isn't Free
Code: 	Etc2u1
Id: 	110686
Prereq: Level 32, Any DoW/DoM

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Amalj'aa Strikers.
SEQ_001	= 1;  -- Talk to Halstein.

-- Actor Class Ids
ENPC_HALSTEIN 			= 1001007;
BNPC_AMALJAA_STRIKER	= 2106541;

-- Quest Markers
MRKR_AMALJAA_AREA	= 11068602;
MRKR_HALSTEIN		= 11068601;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000201;
OBJECTIVE_AMOUNT	= 6;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_HALSTEIN, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_HALSTEIN);
		quest:SetENpc(BNPC_AMALJAA_STRIKER);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_HALSTEIN, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_HALSTEIN and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_HALSTEIN) then
            callClientFunction(player, "delegateEvent", player, quest, "processEventFree");
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_HALSTEIN) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventClear");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_AMALJAA_STRIKER) then
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
		return MRKR_AMALJAA_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_HALSTEIN;
    end
end