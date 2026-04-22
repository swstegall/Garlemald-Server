require ("global")
require ("quest")

--[[

Quest Script

Name: 	Have You Seen My Son
Code: 	Etc1l7
Id: 	110640
Prereq: Level 30, Any DoW/DoM

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Talk to Yuyubesu.
SEQ_001	= 1;  -- Kill Bomb Embers.
SEQ_002	= 2;  -- Return to Yuyubesu.
SEQ_003	= 3;  -- Talk to Hildie.

-- Actor Class Ids
ENPC_IMANIA 		= 1001567;
ENPC_YUYUBESU 		= 1001166;
ENPC_HILDIE 		= 1000787;
BNPC_BOMB_EMBER		= 2101609;

-- Quest Markers
MRKR_YUYUBESU		= 11064001;
MRKR_HILDIE			= 11064002;
MRKR_BOMB_AREA		= 11064003;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000151;
OBJECTIVE_AMOUNT	= 8;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_IMANIA, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
		quest:SetENpc(ENPC_IMANIA);
		quest:SetENpc(ENPC_YUYUBESU, QFLAG_TALK);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_YUYUBESU);
		quest:SetENpc(BNPC_BOMB_EMBER);
	elseif (sequence == SEQ_002) then
		quest:SetENpc(ENPC_YUYUBESU, QFLAG_TALK);
	elseif (sequence == SEQ_003) then
		quest:SetENpc(ENPC_YUYUBESU);
		quest:SetENpc(ENPC_HILDIE, QFLAG_REWARD);
	end				
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_IMANIA and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventImaniaStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
		if (npcClassId == ENPC_IMANIA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventImaniaFree");
        elseif (npcClassId == ENPC_YUYUBESU) then
            callClientFunction(player, "delegateEvent", player, quest, "processEventYuyubesuStart", 0, OBJECTIVE_AMOUNT);
			quest:StartSequence(SEQ_001);
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_YUYUBESU) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventYuyubesuFree", 0, OBJECTIVE_AMOUNT);
		end
	elseif (seq == SEQ_002) then
		if (npcClassId == ENPC_YUYUBESU) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventYuyubesuAfter", 0, OBJECTIVE_AMOUNT);
			quest:StartSequence(SEQ_003);
		end	
	elseif (seq == SEQ_003) then
		if (npcClassId == ENPC_YUYUBESU) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventYuyubesuAfterFree");
		elseif (npcClassId == ENPC_HILDIE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventHildie");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_001 and bnpc == BNPC_BOMB_EMBER) then
		local counterAmount = quest:GetData():IncCounter(COUNTER_QUESTITEM);
		attentionMessage(player, 25226, OBJECTIVE_ITEMID, 1, counterAmount, OBJECTIVE_AMOUNT); -- You obtain <item> (X of Y)
        if (counterAmount >= OBJECTIVE_AMOUNT) then
			attentionMessage(player, 25225, quest:GetQuestId()); -- Objectives complete!
			quest:StartSequence(SEQ_002);
		end
	end
end

function getJournalInformation(player, quest)
	return quest:GetData():GetCounter(COUNTER_QUESTITEM), 0, 0, 0, OBJECTIVE_AMOUNT;
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_YUYUBESU;
    elseif (sequence == SEQ_001) then
        return MRKR_BOMB_AREA;		
    elseif (sequence == SEQ_002) then
        return MRKR_YUYUBESU;		
    elseif (sequence == SEQ_003) then
        return MRKR_HILDIE;
    end
end