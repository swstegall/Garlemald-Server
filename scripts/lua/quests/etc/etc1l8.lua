require ("global")
require ("quest")

--[[

Quest Script

Name: 	Food for Thought
Code: 	Etc1l8
Id: 	110641
Prereq: Level 20, Any Class

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Spread rumors to all, must perform /psych.
SEQ_001	= 1;  -- Return to Dympna.

-- Actor Class Ids
DYMPNA 		= 1000331;
AERGWYNT	= 1000347;
FERDILLAIX 	= 1000344;
BUBUROON	= 1000219;
RBAHARRA 	= 1000340;
FUFUNA 		= 1000345;

-- Quest Markers
MRKR_DYMPNA		= 11064101;
MRKR_AERGWYNT	= 11064102;
MRKR_FERDILLAIX	= 11064103;
MRKR_BUBUROON	= 11064104;
MRKR_RBAHARRA	= 11064105;
MRKR_FUFUNA		= 11064106;

-- Quest Flags
FLAG_TALKED_AERGWYNT 	= 0;
FLAG_TALKED_FERDILLAIX 	= 1;
FLAG_TALKED_BUBUROON 	= 2;
FLAG_TALKED_RBAHARRA	= 3;
FLAG_TALKED_FUFUNA 		= 4;

-- Quest Counters
COUNTER_TALKED          = 0;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(DYMPNA, QFLAG_TALK);
	end

	local data = quest:GetData();
	if (sequence == SEQ_000) then
        quest:SetENpc(DYMPNA);
		quest:SetENpc(AERGWYNT,     (not data:GetFlag(FLAG_TALKED_AERGWYNT) and QFLAG_TALK or QFLAG_NONE), true, false, true);
		quest:SetENpc(FERDILLAIX,   (not data:GetFlag(FLAG_TALKED_FERDILLAIX) and QFLAG_TALK or QFLAG_NONE), true, false, true);
		quest:SetENpc(BUBUROON,		(not data:GetFlag(FLAG_TALKED_BUBUROON) and QFLAG_TALK or QFLAG_NONE), true, false, true);
		quest:SetENpc(RBAHARRA,     (not data:GetFlag(FLAG_TALKED_RBAHARRA) and QFLAG_TALK or QFLAG_NONE), true, false, true);
		quest:SetENpc(FUFUNA,       (not data:GetFlag(FLAG_TALKED_FUFUNA) and QFLAG_TALK or QFLAG_NONE), true, false, true);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(DYMPNA, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == DYMPNA and not player:HasQuest(quest)) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventOffersStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;
	end
	
	-- Quest Progress
	local data = quest:GetData();
	if (seq == SEQ_000) then
        if (npcClassId == DYMPNA) then
            callClientFunction(player, "delegateEvent", player, quest, "processEventOffersAfter");
		elseif (npcClassId == AERGWYNT) then
			if (not data:GetFlag(FLAG_TALKED_AERGWYNT)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventAergwyntSpeak");
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEventAergwyntAfter");
			end
		elseif (npcClassId == FERDILLAIX) then
			if (not data:GetFlag(FLAG_TALKED_FERDILLAIX)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventFerdillaixSpeak");
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEventFerdillaixAfter");
			end
		elseif (npcClassId == BUBUROON) then
			if (not data:GetFlag(FLAG_TALKED_BUBUROON)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventBuburoonSpeak");
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEventBuburoonAfter");
			end
		elseif (npcClassId == RBAHARRA) then
			if (not data:GetFlag(FLAG_TALKED_RBAHARRA)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventBaharraSpeak");
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEventBaharraAfter");
			end
		elseif (npcClassId == FUFUNA) then
			if (not data:GetFlag(FLAG_TALKED_FUFUNA)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventFufunaSpeak");
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEventFufunaAfter");
			end
		end
	elseif (seq == SEQ_001) then
		--Quest Complete
		if (npcClassId == DYMPNA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventClear");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onEmote(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
	local data = quest:GetData();
	local incCounter = false;
	
	-- Play the emote
	if (eventName == "emoteDefault1") then -- Psych
		player:DoEmote(npc.Id, 30, 21291);
	end
	wait(2.5);
	
	-- Handle the result
	if (seq == SEQ_000 and eventName == "emoteDefault1") then
		if (npcClassId == AERGWYNT) then
			if (not data:GetFlag(FLAG_TALKED_AERGWYNT)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventAergwynt");
				data:SetFlag(FLAG_TALKED_AERGWYNT);
                incCounter = true;
			end
		elseif (npcClassId == FERDILLAIX) then
			if (not data:GetFlag(FLAG_TALKED_FERDILLAIX)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventFerdillaix");
				data:SetFlag(FLAG_TALKED_FERDILLAIX);	
                incCounter = true;
			end
		elseif (npcClassId == BUBUROON) then
			if (not data:GetFlag(FLAG_TALKED_BUBUROON)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventBuburoon");
				data:SetFlag(FLAG_TALKED_BUBUROON);
               incCounter = true;
			end
		elseif (npcClassId == RBAHARRA) then
			if (not data:GetFlag(FLAG_TALKED_RBAHARRA)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventBaharra");
				data:SetFlag(FLAG_TALKED_RBAHARRA);
                incCounter = true;                			
			end
		elseif (npcClassId == FUFUNA) then
			if (not data:GetFlag(FLAG_TALKED_FUFUNA)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventFufuna");
				data:SetFlag(FLAG_TALKED_FUFUNA);	
                incCounter = true;			
			end
		end
		  
		-- Increase objective counter & play relevant messages
		if (incCounter == true) then
            local counterAmount = data:IncCounter(COUNTER_TALKED);

            attentionMessage(player, 51059, 0, counterAmount, 5); -- You have helped spread Dympna's rumor. (... of 5)
            
            if (seq000_checkCondition(data)) then -- All informants spoken to
                attentionMessage(player, 25225, quest.GetQuestId()); -- objectives complete!
                quest:UpdateENPCs(); -- Band-aid for a QFLAG_TALK issue
                quest:StartSequence(SEQ_001);
            end
        end       
	end

	quest:UpdateENPCs();	
	player:EndEvent();
end

-- Check if all informants are talked to
function seq000_checkCondition(data)
	return (data:GetFlag(FLAG_TALKED_AERGWYNT) and
			data:GetFlag(FLAG_TALKED_FERDILLAIX) and
			data:GetFlag(FLAG_TALKED_BUBUROON) and
			data:GetFlag(FLAG_TALKED_RBAHARRA) and
			data:GetFlag(FLAG_TALKED_FUFUNA));
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
	local data = quest:GetData();
    local possibleMarkers = {};
    
    if (sequence == SEQ_000) then
        if (not data:GetFlag(FLAG_TALKED_AERGWYNT)) then table.insert(possibleMarkers, MRKR_AERGWYNT); end
        if (not data:GetFlag(FLAG_TALKED_FERDILLAIX)) then table.insert(possibleMarkers, MRKR_FERDILLAIX); end
        if (not data:GetFlag(FLAG_TALKED_BUBUROON)) then table.insert(possibleMarkers, MRKR_BUBUROON); end
        if (not data:GetFlag(FLAG_TALKED_RBAHARRA)) then table.insert(possibleMarkers, MRKR_RBAHARRA); end
        if (not data:GetFlag(FLAG_TALKED_FUFUNA)) then table.insert(possibleMarkers, MRKR_FUFUNA); end
    elseif (sequence == SEQ_001) then
        table.insert(possibleMarkers, MRKR_DYMPNA);
    end
    
    return unpack(possibleMarkers)
end