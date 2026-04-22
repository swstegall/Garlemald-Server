require ("global")
require ("quest")

--[[

Quest Script

Name: 	Of Archons and Muses
Code: 	Wld0u1
Id: 	110753 
Prereq: Level 10, Any Class

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Talk to Lyngwaek.
SEQ_001	= 1;  -- Talk to Tyago Moui.

-- Actor Class Ids
TYAGO_MOUI	 	= 1001203;
LYNGWAEK 		= 1000647;

-- Quest Markers
MRKR_LYNGWAEK	= 11130001;
MRKR_TYAGO_MOUI	= 11130002;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(TYAGO_MOUI, QFLAG_TALK);
	end

	if (sequence == SEQ_000) then
        quest:SetENpc(TYAGO_MOUI);
		quest:SetENpc(LYNGWAEK, QFLAG_TALK);
	elseif (sequence == SEQ_001) then	
		quest:SetENpc(LYNGWAEK);
		quest:SetENpc(TYAGO_MOUI, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == TYAGO_MOUI and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventTyagomouiStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;
	end
	
	-- Quest Progress
	if (seq == SEQ_000) then
        if (npcClassId == TYAGO_MOUI) then
            callClientFunction(player, "delegateEvent", player, quest, "followEvent005");
		elseif (npcClassId == LYNGWAEK) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
			quest:StartSequence(SEQ_001);
		end
	elseif (seq == SEQ_001) then
		--Quest Complete
		if (npcClassId == TYAGO_MOUI) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		elseif (npcClassId == LYNGWAEK) then
			callClientFunction(player, "delegateEvent", player, quest, "followEvent015");
			quest:StartSequence(SEQ_001);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_LYNGWAEK;
    elseif (sequence == SEQ_001) then
        return MRKR_TYAGO_MOUI;
    end
end