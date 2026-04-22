require ("global")
require ("quest")

--[[

Quest Script

Name: 	Embarrassing Excerpts
Code: 	Etc1g9
Id: 	110663
Prereq: Level 30, Any DoW/DoM

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Opo Opos.
SEQ_001	= 1;  -- Talk to Lonsygg.

-- Actor Class Ids
ENPC_LONSYGG 		= 1000951;
BNPC_OPO_OPO		= 2100503;

-- Quest Markers
MRKR_OPO_OPO_AREA	= 11066301;
MRKR_LONSYGG		= 11066302;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000160;
OBJECTIVE_AMOUNT	= 5;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_LONSYGG, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_LONSYGG);
		quest:SetENpc(BNPC_OPO_OPO);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_LONSYGG, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_LONSYGG and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventLonsyggStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_LONSYGG) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent005_2");
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_LONSYGG) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_OPO_OPO) then
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
		return MRKR_OPO_OPO_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_LONSYGG;
    end
end