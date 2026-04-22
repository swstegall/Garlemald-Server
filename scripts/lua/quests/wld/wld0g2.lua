require ("global")
require ("quest")

--[[

Quest Script

Name:   Hearing Confession
Code:   Wld0g2
Id:     110763
Prereq: Level 10 on any class.
Notes:  Rewards 200 gil

]]

-- Sequence Numbers
SEQ_000 = 0;  -- Talk to the four lost souls.
SEQ_001 = 1;  -- Return to Swaenhylt

-- Actor Class Ids
SWAENHYLT	= 1001582;
FLAVIELLE   = 1001459;
KEKETO		= 1001346;
CEADDA		= 1000330;
THIMM		= 1001439;

-- Quest Markers
MRKR_FLAVIELLE			= 11120101;
MRKR_KEKETO				= 11120102;
MRKR_CEADDA				= 11120103;
MRKR_THIMM				= 11120104;
MRKR_SWAENHYLT			= 11120105;

-- Quest Flags
FLAG_TALKED_FLAVIELLE 	= 0;
FLAG_TALKED_KEKETO 		= 1;
FLAG_TALKED_CEADDA 		= 2;
FLAG_TALKED_THIMM	 	= 3;

-- Quest Counters
COUNTER_TALKED          = 0;

function onStart(player, quest)
    quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(SWAENHYLT, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
		local data = quest:GetData();
        quest:SetENpc(SWAENHYLT);
		quest:SetENpc(FLAVIELLE,    (not data:GetFlag(FLAG_TALKED_FLAVIELLE) and QFLAG_TALK or QFLAG_NONE));
		quest:SetENpc(KEKETO,   	(not data:GetFlag(FLAG_TALKED_KEKETO) and QFLAG_TALK or QFLAG_NONE));
		quest:SetENpc(CEADDA,       (not data:GetFlag(FLAG_TALKED_CEADDA) and QFLAG_TALK or QFLAG_NONE));
		quest:SetENpc(THIMM,       	(not data:GetFlag(FLAG_TALKED_THIMM) and QFLAG_TALK or QFLAG_NONE));
    elseif (sequence == SEQ_001) then 
        quest:SetENpc(SWAENHYLT, QFLAG_REWARD);
    end
end

function onTalk(player, quest, npc)
    local sequence = quest:getSequence();
    local classId = npc:GetActorClassId();
    
	if (sequence == SEQ_ACCEPT) then		
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventSwaenhyltStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	elseif (sequence == SEQ_000) then
		local incCounter = false;
		local data = quest:GetData();
		
        if (classId == SWAENHYLT) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent000_2");
        elseif (classId == FLAVIELLE) then
			if (not data:GetFlag(FLAG_TALKED_FLAVIELLE)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent005");
				data:SetFlag(FLAG_TALKED_FLAVIELLE);
                incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent005_2");
			end
		elseif (classId == KEKETO) then
			if (not data:GetFlag(FLAG_TALKED_KEKETO)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
				data:SetFlag(FLAG_TALKED_KEKETO);
                incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent010_2");
			end
		elseif (classId == CEADDA) then
			if (not data:GetFlag(FLAG_TALKED_CEADDA)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent015");
				data:SetFlag(FLAG_TALKED_CEADDA);
                incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent015_2");
			end
		elseif (classId == THIMM) then
			if (not data:GetFlag(FLAG_TALKED_THIMM)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent020");
				data:SetFlag(FLAG_TALKED_THIMM);
                incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent020_2");
			end
		end
		
		-- Increase objective counter & play relevant messages
		if (incCounter == true) then
            local counterAmount = data:IncCounter(COUNTER_TALKED);

            attentionMessage(player, 51063, 0, counterAmount, 4); -- You have heard a lost soul's confession. (... of 5)
            
            if (seq000_checkCondition(data)) then -- All lost souls spoken to
                attentionMessage(player, 25225, quest:GetQuestId()); -- "Hearing Confessions" objectives complete!
                quest:UpdateENPCs(); -- Band-aid for a QFLAG_TALK issue
                quest:StartSequence(SEQ_001);
            end
        end
    elseif (sequence == SEQ_001) then
        if (classId == SWAENHYLT) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent025");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
			player:CompleteQuest(quest);        
        end  
    end
    player:EndEvent()
	quest:UpdateENPCs();
end

-- Check if all souls are talked to
function seq000_checkCondition(data)	
	return (data:GetFlag(FLAG_TALKED_FLAVIELLE) and
			data:GetFlag(FLAG_TALKED_KEKETO) and
			data:GetFlag(FLAG_TALKED_CEADDA) and
			data:GetFlag(FLAG_TALKED_THIMM));
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
	local data = quest:GetData();
    local possibleMarkers = {};
    
    if (sequence == SEQ_000) then
        if (not data:GetFlag(FLAG_TALKED_FLAVIELLE)) then table.insert(possibleMarkers, MRKR_FLAVIELLE); end
        if (not data:GetFlag(FLAG_TALKED_KEKETO)) then table.insert(possibleMarkers, MRKR_KEKETO); end
        if (not data:GetFlag(FLAG_TALKED_CEADDA)) then table.insert(possibleMarkers, MRKR_CEADDA); end
        if (not data:GetFlag(FLAG_TALKED_THIMM)) then table.insert(possibleMarkers, MRKR_THIMM); end
    elseif (sequence == SEQ_001) then
        table.insert(possibleMarkers, MRKR_SWAENHYLT);
    end
    
    return unpack(possibleMarkers)
end



