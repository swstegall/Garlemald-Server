require ("global")
require ("quest")

--[[

Quest Script

Name: 	Sanguine Studies
Code: 	Wld0u2
Id: 	110754
Prereq: Level 27, Any Class

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Amal'jaa Grunts
SEQ_001	= 1;  -- Talk to Papala.

-- Actor Class Ids
ENPC_PAPALA 		= 1001316;
BNPC_AMALJAA_GRUNTS	= 2106537;

-- Quest Markers
MRKR_PAPALA			= 11130101;
MRKR_AMALJAA_GRUNTS	= 11130102;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000173;
OBJECTIVE_AMOUNT	= 3;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_PAPALA, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_PAPALA);
		quest:SetENpc(BNPC_AMALJAA_GRUNTS);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_PAPALA, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_PAPALA and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventPapalaStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_PAPALA) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent000");
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_PAPALA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_AMALJAA_GRUNTS) then
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
		return MRKR_AMALJAA_GRUNTS;
    elseif (sequence == SEQ_001) then
        return MRKR_PAPALA;
    end
end