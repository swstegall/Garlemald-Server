require ("global")
require ("quest")

--[[

Quest Script

Name: 	Sanguine Studies
Code: 	Wld0u4
Id: 	1107546
Prereq: Level 28, Any Class, Sanguine Studies completed.

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Amal'jaa Drudges
SEQ_001	= 1;  -- Talk to Papala.

-- Actor Class Ids
ENPC_PAPALA 		 = 1001316;
BNPC_AMALJAA_DRUDGES = 2106542;

-- Quest Markers
MRKR_AMALJAA_DRUDGES = 11130301;
MRKR_PAPALA			 = 11130302;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000303;
OBJECTIVE_AMOUNT	= 6;

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
		quest:SetENpc(BNPC_AMALJAA_DRUDGES);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_PAPALA, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_PAPALA and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventPAPALAStart", OBJECTIVE_AMOUNT);
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_PAPALA) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_000_1", OBJECTIVE_AMOUNT);
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_PAPALA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent_010");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_AMALJAA_DRUDGES) then
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
		return MRKR_AMALJAA_DRUDGES;
    elseif (sequence == SEQ_001) then
        return MRKR_PAPALA;
    end
end