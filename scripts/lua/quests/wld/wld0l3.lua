require ("global")
require ("quest")

--[[

Quest Script

Name: 	Sniffing Out a Profit
Code: 	Wld0l3
Id: 	110773
Prereq: Level 17, Any Class

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Talk to Lolojo.
SEQ_001	= 1;  -- Talk to Syzfrusk.

-- Actor Class Ids
SYZFRUSK 	= 1001306;
LOLOJO	 	= 1001603;

-- Quest Markers
MRKR_LOLOJO		= 11110201;
MRKR_SYZFRUSK	= 11110202;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(SYZFRUSK, QFLAG_TALK);
	end

	if (sequence == SEQ_000) then
        quest:SetENpc(SYZFRUSK);
		quest:SetENpc(LOLOJO, QFLAG_TALK);
	elseif (sequence == SEQ_001) then	
		quest:SetENpc(LOLOJO);
		quest:SetENpc(SYZFRUSK, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == SYZFRUSK and not player:HasQuest(quest)) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventOffersStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;
	end
	
	-- Quest Progress
	if (seq == SEQ_000) then
        if (npcClassId == SYZFRUSK) then
            callClientFunction(player, "delegateEvent", player, quest, "processEventFree");
		elseif (npcClassId == LOLOJO) then
			callClientFunction(player, "delegateEvent", player, quest, "processlolojoEvent");
			quest:StartSequence(SEQ_001);
		end
	elseif (seq == SEQ_001) then
		--Quest Complete
		if (npcClassId == SYZFRUSK) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventClear");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		elseif (npcClassId == LOLOJO) then
			callClientFunction(player, "delegateEvent", player, quest, "processlolojoEventFree");
			quest:StartSequence(SEQ_001);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_LOLOJO;
    elseif (sequence == SEQ_001) then
        return MRKR_SYZFRUSK;
    end
end