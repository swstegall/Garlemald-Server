require ("global")
require ("quest")

--[[

Quest Script

Name: 	The Penultimate Prank
Code: 	Etc1g4
Id: 	110658
Prereq: Level 30, Any DoW/DoM

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Aurora Anglers.
SEQ_001	= 1;  -- Talk to Nicoliaux.

-- Actor Class Ids
ENPC_NICOLIAUX 		= 1000409;
BNPC_AURORA_ANGLER	= 2104508;

-- Quest Markers
MRKR_ANGLER_AREA	= 11065801;
MRKR_NICOLIAUX		= 11065802;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000143;
OBJECTIVE_AMOUNT	= 8;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_NICOLIAUX, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_NICOLIAUX);
		quest:SetENpc(BNPC_MUSK_ANGLER);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_NICOLIAUX, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_NICOLIAUX and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventNicoliauxStart", 0, OBJECTIVE_AMOUNT);
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_NICOLIAUX) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent010", 0, OBJECTIVE_AMOUNT);
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_NICOLIAUX) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_AURORA_ANGLER) then
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
		return MRKR_ANGLER_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_NICOLIAUX;
    end
end