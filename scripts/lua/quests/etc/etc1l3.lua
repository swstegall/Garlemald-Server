require ("global")
require ("quest")

--[[

Quest Script

Name: 	Revenge on the Reavers
Code: 	Etc1l3
Id: 	110636
Prereq: Level 45, Any DoW/DoM

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Reavers.
SEQ_001	= 1;  -- Talk to Chaunollet.

-- Actor Class Ids
ENPC_CHAUNOLLET		= 1000125;
BNPC_REAVER_CLAWS	= 2180301;
BNPC_REAVER_FINS	= 2180302;
BNPC_REAVER_EYES	= 2180303;

-- Quest Markers
MRKR_REAVER_AREA	= 11063601;
MRKR_CHAUNOLLET		= 11063602;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000148;
OBJECTIVE_AMOUNT	= 8;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_CHAUNOLLET, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_CHAUNOLLET);
		quest:SetENpc(BNPC_REAVER_EYES);
		quest:SetENpc(BNPC_REAVER_FINS);
		quest:SetENpc(BNPC_REAVER_CLAWS);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_CHAUNOLLET, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_CHAUNOLLET and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventChaunolletStart", OBJECTIVE_AMOUNT);
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_CHAUNOLLET) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent000Chaunollet", OBJECTIVE_AMOUNT);
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_CHAUNOLLET) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010Chaunollet");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (bnpc == BNPC_REAVER_EYES or bnpc == BNPC_REAVER_FINS or bnpc == BNPC_REAVER_CLAWS) then
		local counterAmount = quest:GetData():IncCounter(COUNTER_QUESTITEM);
		attentionMessage(player, 25246, OBJECTIVE_ITEMID, 1); -- You obtain <item>
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
		return MRKR_REAVER_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_CHAUNOLLET;
    end
end