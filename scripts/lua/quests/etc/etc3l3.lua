require ("global")
require ("quest")

--[[

Quest Script

Name: 	What a Pirate Wants
Code: 	Etc3l3
Id: 	110746
Prereq: Level 15, Any Class

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Talk to Tefh Moshroca.
SEQ_001 = 1;  -- Talk to Hasthwab.

-- Actor Class Ids
BADERON			= 1000137;
TEFH_MOSHROCA	= 1000131;
HASTHWAB		= 1001064;

-- Quest Markers
MRKR_MOSHROCA	= 11070301;
MRKR_HATHWAB	= 11070302;

-- Quest Misc
RUM_ITEMID		= 11000207;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(BADERON, QFLAG_TALK);
	end

	if (sequence == SEQ_000) then
		quest:SetENpc(TEFH_MOSHROCA, QFLAG_TALK);
        quest:SetENpc(BADERON);
	elseif (sequence == SEQ_001) then	
		quest:SetENpc(HASTHWAB, QFLAG_REWARD);
		quest:SetENpc(TEFH_MOSHROCA);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == BADERON and not player:HasQuest(quest)) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventBADERONStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;
	end
	
	-- Quest Progress
	if (seq == SEQ_000) then
        if (npcClassId == TEFH_MOSHROCA) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_005");
			player:SendGameMessage(GetWorldMaster(), 25117, 0x20, RUM_ITEMID); -- You obtain a bottle of Radz-at-Han Reserve.
			quest:StartSequence(SEQ_001);
		elseif (npcClassId == BADERON) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_000");
		end
	elseif (seq == SEQ_001) then		
		--Quest Complete
		if (npcClassId == HASTHWAB) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent_010");			
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
			player:CompleteQuest(quest);
		elseif (npcClassId == TEFH_MOSHROCA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent_005_01");
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function getJournalInformation(player, quest)
	return quest:GetSequence() == SEQ_001 and 1 or 0;
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_MOSHROCA;
    elseif (sequence == SEQ_001) then
        return MRKR_HATHWAB;
    end
end