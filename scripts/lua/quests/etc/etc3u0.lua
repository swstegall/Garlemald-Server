require ("global")
require ("quest")

--[[

Quest Script

Name: 	A Call to Arms
Code: 	Etc3u0
Id: 	110695
Prereq: Level 5, Any Class

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Talk to all the ___.
SEQ_001	= 1;  -- Return to Fruhybolg.

-- Actor Class Ids
FRUHYBOLG 		= 1000964;
ZOENGTERBIN 	= 1000784;
LETTICE 		= 1000788;
THIMM 			= 1001439;
VANNES 			= 1001464;
JEGER			= 1000655;

-- Quest Markers
MRKR_FRUHYBOLG		= 11090001;
MRKR_ZOENGTERBIN	= 11090002;
MRKR_LETTICE		= 11090003;
MRKR_THIMM			= 11090004;
MRKR_VANNES			= 11090005;
MRKR_JEGER			= 11090006;

-- Quest Flags
FLAG_TALKED_VANNES 			= 1;
FLAG_TALKED_JEGER	 		= 2;
FLAG_TALKED_LETTICE		 	= 3;
FLAG_TALKED_ZOENGTERBIN		= 4;
FLAG_TALKED_THIMM	 		= 5;

-- Quest Counters
COUNTER_TALKED              = 0;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(FRUHYBOLG, QFLAG_TALK);
	end

	local data = quest:GetData();
	if (sequence == SEQ_000) then
        quest:SetENpc(FRUHYBOLG);
		quest:SetENpc(VANNES,       (not data:GetFlag(FLAG_TALKED_VANNES) and QFLAG_TALK or QFLAG_NONE));
		quest:SetENpc(JEGER,   		(not data:GetFlag(FLAG_TALKED_JEGER) and QFLAG_TALK or QFLAG_NONE));
		quest:SetENpc(LETTICE,      (not data:GetFlag(FLAG_TALKED_LETTICE) and QFLAG_TALK or QFLAG_NONE));
		quest:SetENpc(ZOENGTERBIN,  (not data:GetFlag(FLAG_TALKED_ZOENGTERBIN) and QFLAG_TALK or QFLAG_NONE));
		quest:SetENpc(THIMM,        (not data:GetFlag(FLAG_TALKED_THIMM) and QFLAG_TALK or QFLAG_NONE));
	elseif (sequence == SEQ_001) then
		quest:SetENpc(FRUHYBOLG, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
	local incCounter = false;
    
	-- Offer the quest
	if (npcClassId == FRUHYBOLG and not player:HasQuest(quest)) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventFhruybolgStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;
	end
	
	-- Quest Progress
	local data = quest:GetData();
	if (seq == SEQ_000) then
        if (npcClassId == FRUHYBOLG) then
            callClientFunction(player, "delegateEvent", player, quest, "processEventOffersAfter");
		elseif (npcClassId == VANNES) then
			if (not data:GetFlag(FLAG_TALKED_VANNES)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent005_V");
				data:SetFlag(FLAG_TALKED_VANNES);
                incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent005_V_2");
			end
		elseif (npcClassId == JEGER) then
			if (not data:GetFlag(FLAG_TALKED_JEGER)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent005_J");
				data:SetFlag(FLAG_TALKED_JEGER);
               incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent005_J_2");
			end
		elseif (npcClassId == LETTICE) then
			if (not data:GetFlag(FLAG_TALKED_LETTICE)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent005_L");
				data:SetFlag(FLAG_TALKED_LETTICE);	
                incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent005_L_2");
			end
		elseif (npcClassId == ZOENGTERBIN) then
			if (not data:GetFlag(FLAG_TALKED_ZOENGTERBIN)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent005_Z");
				data:SetFlag(FLAG_TALKED_ZOENGTERBIN);
                incCounter = true;                
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent005_Z_2");
			end
		elseif (npcClassId == THIMM) then
			if (not data:GetFlag(FLAG_TALKED_THIMM)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent005_T");
				data:SetFlag(FLAG_TALKED_THIMM);	
                incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent005_T_2");
			end
		end
		        
		-- Increase objective counter & play relevant messages
		if (incCounter == true) then
            local counterAmount = data:IncCounter(COUNTER_TALKED);

            attentionMessage(player, 51062, 0, counterAmount, 5); -- You have passed on word of the rite. (... of 5)
            
            if (seq000_checkCondition(data)) then -- All people spoken to
                attentionMessage(player, 25225, quest:GetQuestId()); -- "A Call to Arms" objectives complete!
                quest:UpdateENPCs(); -- Band-aid for a QFLAG_TALK issue
                quest:StartSequence(SEQ_001);
            end
        end
        
	elseif (seq == SEQ_001) then
		--Quest Complete
		if (npcClassId == FRUHYBOLG) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	quest:UpdateENPCs();	
	player:EndEvent();
end


-- Check if all people are talked to
function seq000_checkCondition(data)
	return (data:GetFlag(FLAG_TALKED_VANNES) and
			data:GetFlag(FLAG_TALKED_JEGER) and
			data:GetFlag(FLAG_TALKED_LETTICE) and
			data:GetFlag(FLAG_TALKED_ZOENGTERBIN) and
			data:GetFlag(FLAG_TALKED_THIMM));
end


function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
	local data = quest:GetData();
    local possibleMarkers = {};
    
    if (sequence == SEQ_000) then
        if (not data:GetFlag(FLAG_TALKED_VANNES)) then table.insert(possibleMarkers, MRKR_VANNES); end
        if (not data:GetFlag(FLAG_TALKED_JEGER)) then table.insert(possibleMarkers, MRKR_JEGER); end
        if (not data:GetFlag(FLAG_TALKED_LETTICE)) then table.insert(possibleMarkers, MRKR_LETTICE); end
        if (not data:GetFlag(FLAG_TALKED_ZOENGTERBIN)) then table.insert(possibleMarkers, MRKR_ZOENGTERBIN); end
        if (not data:GetFlag(FLAG_TALKED_THIMM)) then table.insert(possibleMarkers, MRKR_THIMM); end
    elseif (sequence == SEQ_001) then
        table.insert(possibleMarkers, MRKR_FRUHYBOLG);
    end
    
    return unpack(possibleMarkers)
end