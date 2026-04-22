require ("global")
require ("quest")

--[[

Quest Script

Name: 	The Customer Comes First
Code: 	Etc1u4
Id: 	110679
Prereq: Level 30, Any DoW/DoM

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Stormcry monsters.
SEQ_001	= 1;  -- Talk to Cahernaut.
SEQ_002 = 2;  -- Talk to Cahernaut.

-- Actor Class Ids
ENPC_CAHERNAUT 		= 1000915;
ENPC_HALDBERK 		= 1000160;
BNPC_STORMCRY_QUARTERMASTER	= 2180210;
BNPC_STORMCRY_BOATSWAIN		= 2180211;
BNPC_STORMCRY_POWDER_MONKEY	= 2180212;

-- Quest Markers
MRKR_HALDBERK	= 11067901;
MRKR_STORMCRY	= 11067902;
MRKR_HALDBERK2	= 11067903;

-- Counters
COUNTER_QUESTITEM	= 0;

-- Quest Details
OBJECTIVE_ITEMID	= 11000156;
OBJECTIVE_AMOUNT	= 8;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_CAHERNAUT, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
		quest:SetENpc(ENPC_HALDBERK, QFLAG_TALK);
        quest:SetENpc(ENPC_CAHERNAUT);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(BNPC_STORMCRY_QUARTERMASTER);
		quest:SetENpc(BNPC_STORMCRY_BOATSWAIN);
		quest:SetENpc(BNPC_STORMCRY_POWDER_MONKEY);
		quest:SetENpc(ENPC_HALDBERK);
        quest:SetENpc(ENPC_CAHERNAUT);
	elseif (sequence == SEQ_002) then
		quest:SetENpc(ENPC_HALDBERK, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_CAHERNAUT and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventCahernautStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_CAHERNAUT) then
            callClientFunction(player, "delegateEvent", player, quest, "processEventCahernautFollow");
		elseif (npcClassId == ENPC_HALDBERK) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005_1", OBJECTIVE_AMOUNT);
			quest:StartSequence(SEQ_001);
		end
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_HALDBERK) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005_1Follow");
		elseif (npcClassId == ENPC_CAHERNAUT) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent005Cahernaut");
		end
	--Quest Complete
	elseif (seq == SEQ_002) then
		if (npcClassId == ENPC_HALDBERK) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent010_1");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (quest:GetSequence() == SEQ_001 and (bnpc == BNPC_STORMCRY_BOATSWAIN or bnpc == BNPC_STORMCRY_POWDER_MONKEY or bnpc == BNPC_STORMCRY_QUARTERMASTER)) then
		local counterAmount = quest:GetData():IncCounter(COUNTER_QUESTITEM);
		attentionMessage(player, 25226, OBJECTIVE_ITEMID, 1, counterAmount, OBJECTIVE_AMOUNT); -- You obtain <item> (X of Y)
        if (counterAmount >= OBJECTIVE_AMOUNT) then
			attentionMessage(player, 25225, quest:GetQuestId()); -- Objectives complete!
			quest:StartSequence(SEQ_002);
		end
	end
end

function getJournalInformation(player, quest)
	return quest:GetData():GetCounter(COUNTER_QUESTITEM);
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_HALDBERK;
    elseif (sequence == SEQ_001) then
        return MRKR_STORMCRY;
	elseif (sequence == SEQ_002) then
		return MRKR_HALDBERK2;
    end
end