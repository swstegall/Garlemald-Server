require ("global")
require ("quest")

--[[

Quest Script

Name: 	The Search for Sicksa
Code: 	Etc1g5
Id: 	110659
Prereq: Level 10, Any Class

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Bristletail Marmots.
SEQ_001	= 1;  -- Talk to Beli.

-- Actor Class Ids
ENPC_BELI 				= 1001077;
BNPC_BRISTLETAIL_MARMOT	= 2104022;

-- Quest Markers
MRKR_BRISTLETAIL_AREA	= 11065901;
MRKR_BELI				= 11065902;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000144;
OBJECTIVE_AMOUNT	= 8;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_BELI, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_BELI);
		quest:SetENpc(BNPC_BRISTLETAIL_MARMOT);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_BELI, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_BELI and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventLahonoStart", 0, OBJECTIVE_AMOUNT);
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_BELI) then
            callClientFunction(player, "delegateEvent", player, quest, "processEventFree", 0, OBJECTIVE_AMOUNT);
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_BELI) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventAfter");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_BRISTLETAIL_MARMOT) then
		local counterAmount = quest:GetData():IncCounter(COUNTER_QUESTITEM);
		attentionMessage(player, 25226, OBJECTIVE_ITEMID, 1, counterAmount, OBJECTIVE_AMOUNT); -- You obtain <item> (X of Y)
        if (counterAmount >= OBJECTIVE_AMOUNT) then
			attentionMessage(player, 25225, quest:GetQuestId()); -- Objectives complete!
			quest:StartSequence(SEQ_001);
		end
	end
end

function getJournalInformation(player, quest)
	return quest:GetData():GetCounter(COUNTER_QUESTITEM), 0, 0, 0, OBJECTIVE_AMOUNT;
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_BRISTLETAIL_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_BELI;
    end
end