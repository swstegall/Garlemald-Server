require ("global")
require ("quest")

--[[

Quest Script

Name: 	Call of Booty
Code: 	Etc303
Id: 	110810
Prereq: Level 15, Any Class. Etc3l3, Etc3g3, or Etc3u3 completed.

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Talk to F'ongho.

-- Actor Class Ids
HASTHWAB	= 1001064;
FONGHO		= 1000367;

-- Quest Markers
MRKR_FONGHO	= 11213201;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(HASTHWAB, QFLAG_TALK);
	end

	if (sequence == SEQ_000) then
		quest:SetENpc(FONGHO, QFLAG_REWARD);
        quest:SetENpc(HASTHWAB);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == HASTHWAB and not player:HasQuest(quest)) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventHASTHWABStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;
	end
	
	if (seq == SEQ_000) then
        if (npcClassId == FONGHO) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_005");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
			player:CompleteQuest(quest);
		elseif (npcClassId == HASTHWAB) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_000");
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function getJournalMapMarkerList(player, quest)
	return MRKR_FONGHO;   
end