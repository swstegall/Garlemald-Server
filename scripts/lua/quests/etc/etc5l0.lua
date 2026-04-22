require ("global")
require ("quest")

--[[

Quest Script

Name:   The Ink Thief 
Code:   Etc5l0
Id:     110838
Prereq: Level 1 on any class.  Second MSQ completed. (110002 Man0l1 / 110006 Man0g1 / 110010 Man0u1)
Notes:  

]]

-- Sequence Numbers
SEQ_000 = 0;  -- Talk to Sweetnix.
SEQ_001 = 1;  -- Return to Mytesyn.

-- Actor Class Ids
MYTESYN                 = 1000167;
SWEETNIX                = 1001573;

-- Quest Item
ITEM_INKWELL            = 11000223;

-- Quest Markers
MRKR_SWEETNIX           = 11072001;
MRKR_MYTESYN            = 11072002;


function onStart(player, quest)
    quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(MYTESYN, QFLAG_TALK);
	end

    if (sequence == SEQ_000) then
        quest:SetENpc(MYTESYN);
        quest:SetENpc(SWEETNIX, QFLAG_TALK);
    elseif (sequence == SEQ_001) then 
        quest:SetENpc(MYTESYN, QFLAG_REWARD);
        quest:SetENpc(SWEETNIX);
    end
end

function onTalk(player, quest, npc)
    local sequence = quest:getSequence();
    local npcClassId = npc:GetActorClassId();
    
	if (sequence == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventMYTESYNStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;
    elseif (sequence == SEQ_000) then
        if (npcClassId == MYTESYN) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_000_1");
        elseif (npcClassId == SWEETNIX) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_010");
            attentionMessage(player, 25246, ITEM_INKWELL, 1);
            attentionMessage(player, 25225, quest:GetQuestId());
            quest:StartSequence(SEQ_001);
        end
       
    elseif (sequence == SEQ_001) then
        if (npcClassId == MYTESYN) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_020");
            callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200,1 ,1)
            player:CompleteQuest(quest);
        elseif (npcClassId == SWEETNIX) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_010_1");
        end  
    end
    player:EndEvent()
	quest:UpdateENPCs();
end

function getJournalInformation(player, quest)
    local sequence = quest:getSequence();    
    if (sequence == SEQ_001) then
        return ITEM_INKWELL;
    end
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
        return MRKR_SWEETNIX;
    elseif (sequence == SEQ_001) then
        return MRKR_MYTESYN;
    end    
end



