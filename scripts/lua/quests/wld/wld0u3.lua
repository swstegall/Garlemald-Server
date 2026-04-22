require ("global")
require ("quest")

--[[

Quest Script

Name: 	Secrets Unearthed
Code: 	Wld0u3
Id: 	110756
Prereq: Level 17, Any Class

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Talk to Mumukiya.
SEQ_001	= 1;  -- Talk to Abelard.

-- Actor Class Ids
MUMUKIYA 		= 1001165;
ABELARD		 	= 1001596;

-- Quest Markers
MRKR_ABELARD	= 11130201;
MRKR_MUMUKIYA	= 11130202;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(MUMUKIYA, QFLAG_TALK);
	end

	if (sequence == SEQ_000) then
        quest:SetENpc(MUMUKIYA);
		quest:SetENpc(ABELARD, QFLAG_TALK);
	elseif (sequence == SEQ_001) then	
		quest:SetENpc(ABELARD);
		quest:SetENpc(MUMUKIYA, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == MUMUKIYA and not player:HasQuest(quest)) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventMUMUKIYAStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;
	end
	
	-- Quest Progress
	if (seq == SEQ_000) then
        if (npcClassId == MUMUKIYA) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_000");
		elseif (npcClassId == ABELARD) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent_010");
			quest:StartSequence(SEQ_001);
		end
	elseif (seq == SEQ_001) then
		--Quest Complete
		if (npcClassId == MUMUKIYA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent_020");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		elseif (npcClassId == ABELARD) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent_010_1");
			quest:StartSequence(SEQ_001);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_ABELARD;
    elseif (sequence == SEQ_001) then
        return MRKR_MUMUKIYA;
    end
end