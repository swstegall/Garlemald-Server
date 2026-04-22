require ("global")
require ("quest")

--[[

Quest Script

Name: 	Besmitten and Besmirched
Code: 	Etc1u6
Id: 	110681
Prereq: Level 15, Any DoW/DoM

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Moiling Moles.
SEQ_001	= 1;  -- Talk to Mohtfryd.

-- Actor Class Ids
ENPC_MOHTFRYD 		= 1001170;
BNPC_MOILING_MOLE	= 2105717;

-- Quest Markers
MRKR_MOLE_AREA		= 11068102;
MRKR_MOHTFRYD		= 11068103;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000157;
OBJECTIVE_AMOUNT	= 8;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_MOHTFRYD, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_MOHTFRYD);
		quest:SetENpc(BNPC_MOILING_MOLE);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_MOHTFRYD, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_MOHTFRYD and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventMohtfrydStart", 0, OBJECTIVE_AMOUNT);
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_MOHTFRYD) then
            callClientFunction(player, "delegateEvent", player, quest, "processEventFree", 0, OBJECTIVE_AMOUNT);
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_MOHTFRYD) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventAfter", 0);
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_MOILING_MOLE) then
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
		return MRKR_MOLE_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_MOHTFRYD;
    end
end