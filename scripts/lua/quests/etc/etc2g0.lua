require ("global")
require ("quest")

--[[

Quest Script

Name: 	A Forbidden Love
Code: 	Etc2g0
Id: 	110664
Prereq: Level 30, Any DoW/DoM, Etc1g9 completed

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Mirror Roselets.
SEQ_001	= 1;  -- Talk to Lonsygg.
SEQ_002	= 2;  -- Talk to Ethelinda.

-- Actor Class Ids
ENPC_ETHELINDA 		= 1001352;
ENPC_LONSYGG 		= 1000951;
BNPC_MIRROR_ROSELET	= 2102708;

-- Quest Markers
MRKR_ROSELET_AREA	= 11066401;
MRKR_LONSYGG		= 11066402;
MRKR_ETHELINDA		= 11066403;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000161;
OBJECTIVE_AMOUNT	= 3;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_ETHELINDA, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_ETHELINDA);
		quest:SetENpc(BNPC_MIRROR_ROSELET);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_LONSYGG, QFLAG_TALK);
		quest:SetENpc(ENPC_ETHELINDA);
	elseif (sequence == SEQ_002) then
		quest:SetENpc(ENPC_ETHELINDA, QFLAG_TALK);
		quest:SetENpc(ENPC_LONSYGG);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_ETHELINDA and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventEthelindaStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_ETHELINDA) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent000_2");
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_LONSYGG) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent000");
			quest:StartSequence(SEQ_002);
		elseif (npcClassId == ENPC_ETHELINDA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent000_2");
		end
	elseif (seq == SEQ_002) then
		if (npcClassId == ENPC_ETHELINDA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		elseif (npcClassId == ENPC_LONSYGG) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005_2");
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_MIRROR_ROSELET) then
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
		return MRKR_ROSELET_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_LONSYGG;
	elseif (sequence == SEQ_002) then
		return MRKR_ETHELINDA;
    end
end