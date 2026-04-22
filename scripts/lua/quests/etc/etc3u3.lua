require ("global")
require ("quest")

--[[

Quest Script

Name: 	Cutthroat Prices
Code: 	Etc3u3
Id: 	110728
Prereq: Level 15, Any Class

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Talk to Stangyth.
SEQ_001 = 1;  -- Talk to Hasthwab.

-- Actor Class Ids
MOMODI			= 1000841;
STANGYTH		= 1500208;
HASTHWAB		= 1001064;

-- Quest Markers
MRKR_STANGYTH	= 11090301;
MRKR_HATHWAB	= 11090302;

-- Quest Misc
AIRSHIP_PASS_ITEMID	= 11000209;

function onStart(player, quest)	
	player:SendGameMessage(GetWorldMaster(), 25117, 0x20, AIRSHIP_PASS_ITEMID); -- You obtain a Airship Pass (ULD-LMS).
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(MOMODI, QFLAG_TALK);
	end

	if (sequence == SEQ_000) then
		quest:SetENpc(STANGYTH, QFLAG_TALK);
        quest:SetENpc(MOMODI);
	elseif (sequence == SEQ_001) then	
		quest:SetENpc(HASTHWAB, QFLAG_REWARD);
		quest:SetENpc(STANGYTH);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == MOMODI and not player:HasQuest(quest)) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventMIOUNNEStart"); -- Not a typo, SE's error copy/pasting tsk tsk.
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;
	end
	
	-- Quest Progress
	if (seq == SEQ_000) then
        if (npcClassId == STANGYTH) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_005");
			quest:StartSequence(SEQ_001);
		elseif (npcClassId == MOMODI) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_000");
		end
	elseif (seq == SEQ_001) then		
		--Quest Complete
		if (npcClassId == HASTHWAB) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent_010");			
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
			player:CompleteQuest(quest);
		elseif (npcClassId == STANGYTH) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent_005_01");
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function getJournalInformation(player, quest)
	return quest:GetSequence() == SEQ_000 and 1 or 0;
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_STANGYTH;
    elseif (sequence == SEQ_001) then
        return MRKR_HATHWAB;
    end
end