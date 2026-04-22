require ("global")
require ("quest")

--[[

Quest Script

Name: 	Beryl Overboard
Code: 	Etc1l6
Id: 	110639
Prereq: Level 20, Any DoW/DoM, Requires Etc1l5

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Beryl Crabs.
SEQ_001	= 1;  -- Talk to Nanapiri.

-- Actor Class Ids
ENPC_NANAPIRI 		= 1000136;
BNPC_BERYL_CRAB 	= 2107613;

-- Quest Markers
MRKR_CRAB_AREA		= 11063901;
MRKR_NANAPIRI		= 11063902;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000150;
OBJECTIVE_AMOUNT	= 8;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_NANAPIRI, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_NANAPIRI);
		quest:SetENpc(BNPC_BERYL_CRAB);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_NANAPIRI, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_NANAPIRI and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventNanapiriStart", OBJECTIVE_AMOUNT);
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_NANAPIRI) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent005_2", OBJECTIVE_AMOUNT);
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_NANAPIRI) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_BERYL_CRAB) then
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
		return MRKR_CRAB_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_NANAPIRI;
    end
end