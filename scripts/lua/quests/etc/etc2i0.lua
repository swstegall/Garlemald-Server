require ("global")
require ("quest")

--[[

Quest Script

Name: 	Counting Sheep
Code: 	Etc2i0
Id: 	110706
Prereq: Level 25, Any DoW/DoM

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Dreadwolves.
SEQ_001	= 1;  -- Talk to Patrick.

-- Actor Class Ids
ENPC_PATRICK 		= 1001358;
BNPC_DREADWOLVES	= 2101403;

-- Quest Markers
MRKR_WOLF_AREA		= 11101901;
MRKR_PATRICK		= 11101902;

-- Counters
COUNTER_KILLS		= 0;

-- Quest Details
OBJECTIVE_AMOUNT	= 4;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_PATRICK, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_PATRICK);
		quest:SetENpc(BNPC_DREADWOLVES);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_PATRICK, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_PATRICK and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventPatrickStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_PATRICK) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent000");
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_PATRICK) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_DREADWOLVES) then
		local counterAmount = quest:GetData():IncCounter(COUNTER_KILLS);
		attentionMessage(player, 50041, 3101403, counterAmount, OBJECTIVE_AMOUNT); -- The <dispName> has been defeated. (X of Y)
        if (counterAmount >= OBJECTIVE_AMOUNT) then
			attentionMessage(player, 25225, quest:GetQuestId()); -- Objectives complete!
			quest:StartSequence(SEQ_001);
		end
	end
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_WOLF_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_PATRICK;
    end
end