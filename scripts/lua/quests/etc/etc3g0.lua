require ("global")
require ("quest")

--[[

Quest Script

Name: 	Seeing the Seers
Code: 	Etc3g0
Id: 	110674
Prereq: Level 5, Any Class

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Talk to all the seers.
SEQ_001	= 1;  -- Return to Kinnison.

-- Actor Class Ids
KINNISON 		= 1001430;
SYBELL 			= 1001437;
KHUMA_MOSHROCA	= 1001081;
NELLAURE 		= 1000821;
MESTONNAUX 		= 1001103;
LEFWYNE 		= 1001396;

-- Quest Markers
MRKR_KINNISON		= 11080001
MRKR_SYBELL			= 11080002
MRKR_KHUMA_MOSHROCA	= 11080003
MRKR_NELLAURE		= 11080004
MRKR_MESTONNAUX		= 11080005
MRKR_LEFWYNE		= 11080006

-- Quest Flags
FLAG_TALKED_MESTONNAUX 		= 0;
FLAG_TALKED_SYBELL 			= 1;
FLAG_TALKED_NELLAURE 		= 2;
FLAG_TALKED_KHUMA_MOSHROCA 	= 3;
FLAG_TALKED_LEFWYNE 		= 4;

-- Quest Counters
COUNTER_TALKED              = 0;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(KINNISON, QFLAG_TALK);
	end

	local data = quest:GetData();
	if (sequence == SEQ_000) then
        quest:SetENpc(KINNISON);
		quest:SetENpc(SYBELL,           (not data:GetFlag(FLAG_TALKED_SYBELL) and QFLAG_TALK or QFLAG_NONE));
		quest:SetENpc(KHUMA_MOSHROCA,   (not data:GetFlag(FLAG_TALKED_KHUMA_MOSHROCA) and QFLAG_TALK or QFLAG_NONE));
		quest:SetENpc(NELLAURE,         (not data:GetFlag(FLAG_TALKED_NELLAURE) and QFLAG_TALK or QFLAG_NONE));
		quest:SetENpc(MESTONNAUX,       (not data:GetFlag(FLAG_TALKED_MESTONNAUX) and QFLAG_TALK or QFLAG_NONE));
		quest:SetENpc(LEFWYNE,          (not data:GetFlag(FLAG_TALKED_LEFWYNE) and QFLAG_TALK or QFLAG_NONE));
	elseif (sequence == SEQ_001) then
		quest:SetENpc(KINNISON, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
	local incCounter = false;
    
	-- Offer the quest
	if (npcClassId == KINNISON and not player:HasQuest(quest)) then
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
        if (npcClassId == KINNISON) then
            callClientFunction(player, "delegateEvent", player, quest, "processEventOffersAfter");
		elseif (npcClassId == SYBELL) then
			if (not data:GetFlag(FLAG_TALKED_SYBELL)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventSybellSpeak");
				data:SetFlag(FLAG_TALKED_SYBELL);
                incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEventSybellSpeakAfter");
			end
		elseif (npcClassId == KHUMA_MOSHROCA) then
			if (not data:GetFlag(FLAG_TALKED_KHUMA_MOSHROCA)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventKhumaSpeak");
				data:SetFlag(FLAG_TALKED_KHUMA_MOSHROCA);
               incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEventKhumaSpeakAfter");
			end
		elseif (npcClassId == NELLAURE) then
			if (not data:GetFlag(FLAG_TALKED_NELLAURE)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventNellaureSpeak");
				data:SetFlag(FLAG_TALKED_NELLAURE);	
                incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEventNellaureSpeakAfter");
			end
		elseif (npcClassId == MESTONNAUX) then
			if (not data:GetFlag(FLAG_TALKED_MESTONNAUX)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventMestonnauxSpeak");
				data:SetFlag(FLAG_TALKED_MESTONNAUX);
                incCounter = true;                
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEventMestonnauxSpeakAfter");
			end
		elseif (npcClassId == LEFWYNE) then
			if (not data:GetFlag(FLAG_TALKED_LEFWYNE)) then
				callClientFunction(player, "delegateEvent", player, quest, "processEventLefwyneSpeak");
				data:SetFlag(FLAG_TALKED_LEFWYNE);	
                incCounter = true;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEventLefwyneSpeakAfter");
			end
		end
		        
		-- Increase objective counter & play relevant messages
		if (incCounter == true) then
            local counterAmount = data:IncCounter(COUNTER_TALKED);

            attentionMessage(player, 51061, 0, counterAmount, 5); -- You have heard word of the Seedseers. (... of 5)
            
            if (seq000_checkCondition(data)) then -- All Seers spoken to
                attentionMessage(player, 25225, quest:GetQuestId()); -- "Seeing the Seers" objectives complete!
                quest:UpdateENPCs(); -- Band-aid for a QFLAG_TALK issue
                quest:StartSequence(SEQ_001);
            end
        end
        
	elseif (seq == SEQ_001) then
		--Quest Complete
		if (npcClassId == KINNISON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventClear");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	quest:UpdateENPCs();	
	player:EndEvent();
end


-- Check if all seers are talked to
function seq000_checkCondition(data)
	return (data:GetFlag(FLAG_TALKED_SYBELL) and
			data:GetFlag(FLAG_TALKED_KHUMA_MOSHROCA) and
			data:GetFlag(FLAG_TALKED_NELLAURE) and
			data:GetFlag(FLAG_TALKED_MESTONNAUX) and
			data:GetFlag(FLAG_TALKED_LEFWYNE));
end


function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
	local data = quest:GetData();
    local possibleMarkers = {};
    local data = quest:GetData();
    
    if (sequence == SEQ_000) then
        if (not data:GetFlag(FLAG_TALKED_SYBELL)) then table.insert(possibleMarkers, MRKR_SYBELL); end
        if (not data:GetFlag(FLAG_TALKED_KHUMA_MOSHROCA)) then table.insert(possibleMarkers, MRKR_KHUMA_MOSHROCA); end
        if (not data:GetFlag(FLAG_TALKED_NELLAURE)) then table.insert(possibleMarkers, MRKR_NELLAURE); end
        if (not data:GetFlag(FLAG_TALKED_MESTONNAUX)) then table.insert(possibleMarkers, MRKR_MESTONNAUX); end
        if (not data:GetFlag(FLAG_TALKED_LEFWYNE)) then table.insert(possibleMarkers, MRKR_LEFWYNE); end
    elseif (sequence == SEQ_001) then
        table.insert(possibleMarkers, MRKR_KINNISON);
    end
    
    return unpack(possibleMarkers)
end