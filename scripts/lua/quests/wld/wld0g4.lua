require ("global")
require ("quest")

--[[

Quest Script

Name: 	Spores on the Brain
Code: 	Wld0g4
Id: 	110765
Prereq: Level 11, Any Class, Requires "In the Name of Science"

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Mature Funguars.
SEQ_001	= 1;  -- Talk to Marcette.

-- Actor Class Ids
ENPC_MARCETTE 		= 1001583;
BNPC_MATURE_FUNGUAR	= 2105916;

-- Quest Markers
MRKR_MARCETTE		= 11120302;
MRKR_FUNGUAR_AREA	= 11120301;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000301;
OBJECTIVE_AMOUNT	= 8;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_MARCETTE, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_MARCETTE);
		quest:SetENpc(BNPC_MATURE_FUNGUAR);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_MARCETTE, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_MARCETTE and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventMarcetteStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_MARCETTE) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent000_2");
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_MARCETTE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_MATURE_FUNGUAR) then
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
		return MRKR_FUNGUAR_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_MARCETTE;
    end
end