require ("global")
require ("quest")

--[[

Quest Script

Name:   Letting Out Orion's Belt
Code:   Wld0l2
Id:     110772
Prereq: Level 10 on any class.
Notes:  Rewards 200 gil

]]

-- Sequence Numbers
SEQ_000 = 0;  -- Talk to the four gourmands.
SEQ_001 = 1;  -- Return to Ahldskyf.

-- Actor Class Ids
AHLDSKYF		= 1000332;
FZHUMII			= 1000226;
SHOSHOMA		= 1000334;
DACA_JINJAHL 	= 1000202;
AENTFOET		= 1000064;

-- Quest Markers
MRKR_FZHUMII		= 11110101;
MRKR_SHOSHOMA		= 11110102;
MRKR_DACA_JINJAHL	= 11110103;
MRKR_AENTFOET		= 11110104;
MRKR_AHLDSKYF		= 11110105;

-- Quest Flags
FLAG_TALKED_FZHUMII	 		= 0;
FLAG_TALKED_SHOSHOMA 		= 1;
FLAG_TALKED_DACA_JINJAHL 	= 2;
FLAG_TALKED_AENTFOET 		= 3;

-- Quest Counters
COUNTER_TALKED          	= 0;

function onStart(player, quest)
    quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(AHLDSKYF, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
		local data = quest:GetData();
        quest:SetENpc(AHLDSKYF);
		quest:SetENpc(FZHUMII,      (not data:GetFlag(FLAG_TALKED_FZHUMII) and QFLAG_TALK or QFLAG_NONE));
		quest:SetENpc(SHOSHOMA,     (not data:GetFlag(FLAG_TALKED_SHOSHOMA) and QFLAG_TALK or QFLAG_NONE));
		quest:SetENpc(DACA_JINJAHL, (not data:GetFlag(FLAG_TALKED_DACA_JINJAHL) and QFLAG_TALK or QFLAG_NONE));
		quest:SetENpc(AENTFOET,   	(not data:GetFlag(FLAG_TALKED_AENTFOET) and QFLAG_TALK or QFLAG_NONE));
    elseif (sequence == SEQ_001) then 
        quest:SetENpc(AHLDSKYF, QFLAG_REWARD);
    end
end

function onTalk(player, quest, npc)
    local sequence = quest:getSequence();
    local classId = npc:GetActorClassId();
    
	if (sequence == SEQ_ACCEPT and classId == AHLDSKYF) then		
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventAhldskyffStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	elseif (sequence == SEQ_000) then
		local incCounter = false;
		local data = quest:GetData();
		
        if (classId == AHLDSKYF) then
            callClientFunction(player, "delegateEvent", player, quest, "processEventAhldskyffStart_1");    
		elseif (classId == FZHUMII) then
			if (not data:GetFlag(FLAG_TALKED_FZHUMII)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent000");
				data:SetFlag(FLAG_TALKED_FZHUMII);
                incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent000_1");
			end
		elseif (classId == SHOSHOMA) then
			if (not data:GetFlag(FLAG_TALKED_SHOSHOMA)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent005");
				data:SetFlag(FLAG_TALKED_SHOSHOMA);
                incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent005_1");
			end    
		elseif (classId == DACA_JINJAHL) then
			if (not data:GetFlag(FLAG_TALKED_DACA_JINJAHL)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
				data:SetFlag(FLAG_TALKED_KEKETO);
                incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent010_1");
			end
		elseif (classId == AENTFOET) then
			if (not data:GetFlag(FLAG_TALKED_AENTFOET)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent015");
				data:SetFlag(FLAG_TALKED_AENTFOET);
                incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent015_1");
			end
		end
		
		-- Increase objective counter & play relevant messages
		if (incCounter == true) then
            local counterAmount = data:IncCounter(COUNTER_TALKED);

            attentionMessage(player, 51063, 0, counterAmount, 4); -- ????. (... of 5)
            
            if (seq000_checkCondition(data)) then -- All lost souls spoken to
                attentionMessage(player, 25225, quest:GetQuestId()); -- "Letting Out Orion's Belt" objectives complete!
                quest:UpdateENPCs(); -- Band-aid for a QFLAG_TALK issue
                quest:StartSequence(SEQ_001);
            end
        end
    elseif (sequence == SEQ_001) then
        if (classId == AHLDSKYF) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent020");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
			player:CompleteQuest(quest);        
        end  
    end
    player:EndEvent()
	quest:UpdateENPCs();
end

-- Check if all souls are talked to
function seq000_checkCondition(data)
	return (data:GetFlag(FLAG_TALKED_FZHUMII) and
			data:GetFlag(FLAG_TALKED_SHOSHOMA) and
			data:GetFlag(FLAG_TALKED_DACA_JINJAHL) and
			data:GetFlag(FLAG_TALKED_AENTFOET));
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
	local data = quest:GetData();
    local possibleMarkers = {};
    
    if (sequence == SEQ_000) then
        if (not data:GetFlag(FLAG_TALKED_FZHUMII)) then table.insert(possibleMarkers, MRKR_FZHUMII); end
        if (not data:GetFlag(FLAG_TALKED_SHOSHOMA)) then table.insert(possibleMarkers, MRKR_SHOSHOMA); end
        if (not data:GetFlag(FLAG_TALKED_DACA_JINJAHL)) then table.insert(possibleMarkers, MRKR_DACA_JINJAHL); end
        if (not data:GetFlag(FLAG_TALKED_AENTFOET)) then table.insert(possibleMarkers, MRKR_AENTFOET); end
    elseif (sequence == SEQ_001) then
        table.insert(possibleMarkers, MRKR_AHLDSKYF);
    end
    
    return unpack(possibleMarkers)
end



