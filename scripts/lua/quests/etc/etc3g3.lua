require ("global")
require ("quest")

--[[

Quest Script

Name: 	A Slippery Stone
Code: 	Etc3g3
Id: 	110737
Prereq: Level 15, Any Class

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Talk to Lionnellais.
SEQ_001 = 1;  -- Talk to Hasthwab.

-- Actor Class Ids
MIOUNNE			= 1000230;
LIONNELLAIS		= 1500055;
HASTHWAB		= 1001064;

-- Quest Markers
MRKR_LIONNELLAIS	= 11080301;
MRKR_HATHWAB		= 11080302;

-- Quest Misc
AIRSHIP_PASS_ITEMID	= 11000208;

function onStart(player, quest)	
	player:SendGameMessage(GetWorldMaster(), 25117, 0x20, AIRSHIP_PASS_ITEMID); -- You obtain a Airship Pass (GRD-LMS).
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(MIOUNNE, QFLAG_TALK);
	end

	if (sequence == SEQ_000) then
		quest:SetENpc(LIONNELLAIS, QFLAG_TALK);
        quest:SetENpc(MIOUNNE);
	elseif (sequence == SEQ_001) then	
		quest:SetENpc(HASTHWAB, QFLAG_REWARD);
		quest:SetENpc(LIONNELLAIS);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == MIOUNNE and not player:HasQuest(quest)) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventMIOUNNEStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;
	end
	
	-- Quest Progress
	if (seq == SEQ_000) then
        if (npcClassId == LIONNELLAIS) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_005");
			quest:StartSequence(SEQ_001);
		elseif (npcClassId == MIOUNNE) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_000");
		end
	elseif (seq == SEQ_001) then		
		--Quest Complete
		if (npcClassId == HASTHWAB) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent_010");			
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
			player:CompleteQuest(quest);
		elseif (npcClassId == LIONNELLAIS) then
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
		return MRKR_LIONNELLAIS;
    elseif (sequence == SEQ_001) then
        return MRKR_HATHWAB;
    end
end