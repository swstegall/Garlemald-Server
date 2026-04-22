require ("global")
require ("quest")

--[[

Quest Script

Name: 	Dressed to Be Killed
Code: 	110677
Id: 	110638
Prereq: Level 45, Any DoW/DoM

]]

-- Sequence Numbers
SEQ_000	= 0;  -- Kill Dapper Cadaver.
SEQ_001	= 1;  -- Talk to Tutubuki.

-- Actor Class Ids
ENPC_TUTUBUKI 		= 1001141;
BNPC_DAPPER_CADAVER	= 2101816;

-- Quest Markers
MRKR_CADAVER_AREA	= 11067701;
MRKR_TUTUBUKI		= 11067702;

-- Quest Details
OBJECTIVE_ITEMID	= 11000155;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_TUTUBUKI, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
        quest:SetENpc(ENPC_TUTUBUKI);
		quest:SetENpc(BNPC_DAPPER_CADAVER);
	elseif (sequence == SEQ_001) then
		quest:SetENpc(ENPC_TUTUBUKI, QFLAG_REWARD);
	end	
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
    
	-- Offer the quest
	if (npcClassId == ENPC_TUTUBUKI and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventTutubukiStart", 0, OBJECTIVE_AMOUNT);
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_TUTUBUKI) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent000_2", 0, OBJECTIVE_AMOUNT);
		end
	--Quest Complete
	elseif (seq == SEQ_001) then
		if (npcClassId == ENPC_TUTUBUKI) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 1, 1, 9);
            player:CompleteQuest(quest);
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onKillBNpc(player, quest, bnpc)
	if (bnpc == BNPC_DAPPER_CADAVER) then
		player:SendGameMessage(GetWorldMaster(), 50041, 0x20, 3101818, 1, 1); -- The <dispName> has been defeated. (X of Y)
		player:SendGameMessage(GetWorldMaster(), 25246, 0x20, OBJECTIVE_ITEMID, 1); -- You obtain <item>
        attentionMessage(player, 25225, quest:GetQuestId()); -- Objectives complete!
		quest:StartSequence(SEQ_001);
	end
end

function getJournalInformation(player, quest)
	return quest:GetData():GetCounter(COUNTER_QUESTITEM);
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    
    if (sequence == SEQ_000) then
		return MRKR_CADAVER_AREA;
    elseif (sequence == SEQ_001) then
        return MRKR_TUTUBUKI;
    end
end