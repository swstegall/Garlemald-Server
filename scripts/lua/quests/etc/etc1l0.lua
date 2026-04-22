require ("global")
require ("quest")

--[[

Quest Script

Name: 	Assessing the Damage
Code: 	Etc1l0
Id: 	110633
Prereq: Level 20, Any Class

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Jetsam Jellies.
SEQ_001	= 1;  -- Talk to Haldberk.

-- Actor Class Ids
ENPC_HALDBERK 		= 1000160;
BNPC_JETSAM_JELLIES	= 2105409;

-- Quest Markers
MRKR_JELLIES_AREA	= 11063301;
MRKR_HALDBERK		= 11063302;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000147;
OBJECTIVE_AMOUNT	= 8;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_HALDBERK, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_HALDBERK);
		quest:SetENpc(BNPC_JETSAM_JELLIES);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_HALDBERK, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_HALDBERK and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventHaldberkStart", OBJECTIVE_AMOUNT);
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_HALDBERK) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent000", 0, OBJECTIVE_AMOUNT);
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_HALDBERK) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010", 2);
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_JETSAM_JELLIES) then
		local counterAmount = quest:GetData():IncCounter(COUNTER_QUESTITEM);
		attentionMessage(player, 25226, OBJECTIVE_ITEMID, 1, counterAmount, OBJECTIVE_AMOUNT); -- You obtain <item> (X of Y)
        if (counterAmount >= OBJECTIVE_AMOUNT) then
			attentionMessage(player, 25225, quest:GetQuestId()); -- Objectives complete!
			quest:StartSequence(SEQ_001);
		end
	end
end

function getJournalInformation(player, quest)
	return 0, quest:GetData():GetCounter(COUNTER_QUESTITEM);
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_JELLIES_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_HALDBERK;
    end
end