require ("global")
require ("quest")

--[[

Quest Script

Name: 	A Knock in the Night
Code: 	Etc1u0
Id: 	110675
Prereq: Level 35, Any DoW/DoM

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Cursed Eyes.
SEQ_001	= 1;  -- Talk to Eleanor.

-- Actor Class Ids
ENPC_ELEANOR 		= 1001565;
BNPC_CURSED_EYES	= 2101711;

-- Quest Markers
MRKR_EYES_AREA		= 11067501;
MRKR_ELEANOR		= 11067502;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000153;
OBJECTIVE_AMOUNT	= 8;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_ELEANOR, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_ELEANOR);
		quest:SetENpc(BNPC_CURSED_EYES);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_ELEANOR, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_ELEANOR and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventEleanorStart", OBJECTIVE_AMOUNT);
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_ELEANOR) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent000", OBJECTIVE_AMOUNT);
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_ELEANOR) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_000 and bnpc == BNPC_CURSED_EYES) then
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
		return MRKR_EYES_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_ELEANOR;
    end
end