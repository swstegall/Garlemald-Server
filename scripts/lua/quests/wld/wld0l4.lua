require ("global")
require ("quest")

--[[

Quest Script

Name: 	Sniffing Out a Profit
Code: 	Wld0l4
Id: 	110774
Prereq: Level 37, Any Class, Requires "Letting Out Orion's Belt"

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Talk to Ryssfloh.
SEQ_001	= 1;  -- Talk to Faine.

-- Actor Class Ids
AHLDSKYF 	= 1000332;
FAINE	 	= 1001608;

-- Quest Markers
MRKR_FAINE		= 11110301;
MRKR_AHLDSKYF	= 11110302;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(AHLDSKYF, QFLAG_TALK);
	end

	if (sequence == SEQ_000) then
        quest:SetENpc(AHLDSKYF);
		quest:SetENpc(FAINE, QFLAG_TALK);
	elseif (sequence == SEQ_001) then	
		quest:SetENpc(FAINE);
		quest:SetENpc(AHLDSKYF, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == AHLDSKYF and not player:HasQuest(quest)) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventAhldskyffStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;
	end
	
	-- Quest Progress
	if (seq == SEQ_000) then
        if (npcClassId == AHLDSKYF) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent000");
		elseif (npcClassId == FAINE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005");
			quest:StartSequence(SEQ_001);
		end
	elseif (seq == SEQ_001) then
		--Quest Complete
		if (npcClassId == AHLDSKYF) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		elseif (npcClassId == FAINE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005_2");
			quest:StartSequence(SEQ_001);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_FAINE;
    elseif (sequence == SEQ_001) then
        return MRKR_AHLDSKYF;
    end
end