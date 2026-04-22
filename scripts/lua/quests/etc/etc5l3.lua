require ("global")
require ("quest")

--[[

Quest Script

Name:   Prophecy Inspection
Code:   Etc5l3
Id:     110841
Prereq: Level 20.  Man5l2 (Mysteries of the Red Moon) complete.  [110840]
Notes: 

]]

-- Sequence Numbers
SEQ_000 = 0;  -- Head to the Coffer & Coffin
SEQ_005 = 5;  -- Help out Alret
SEQ_010 = 10; -- Talk to Alret

-- Actor Class Ids
KOPURU_FUPURU           = 1002047;
VKOROLON                = 1000458;
MYTESYN                 = 1000167;
COFFER_AND_COFFIN_PUSH  = 1090090;
BED_LIMSA               = 1200378;
BED_GRIDANIA            = 1200379;
BED_ULDAH               = 1200380;

HILDIBRAND              = 1001995;
NASHU_MHAKARACCA        = 1001996;
ALRET                   = 1002114;
BOMB_BANE_1             = 1080090;
BOMB_BANE_2             = 1080091;
BOMB_BANE_3             = 1080092;
BOMB_BANE_4             = 1080093;
BOMB_BANE_5             = 1080094;

-- Quest Markers
MRKR_COFFIN             = 11072204;
MRKR_BANE_1             = 11072205;
MRKR_BANE_2             = 11072206;
MRKR_BANE_3             = 11072207;
MRKR_BANE_4             = 11072208;
MRKR_BANE_5             = 11072209;
MRKR_ALRET              = 11072210;

-- Quest Flags
FLAG_SEQ005_BANE_1      = 0;
FLAG_SEQ005_BANE_2      = 1;
FLAG_SEQ005_BANE_3      = 2;
FLAG_SEQ005_BANE_4      = 3;
FLAG_SEQ005_BANE_5      = 4;

-- Quest Counter
COUNTER_BANE            = 0;

-- Quest Item
ITEM_HIDLIBRAND_DOSSIER = 10011252;
ITEM_BOMB_BANE          = 11000230;



function onStart(player, quest)
    quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end


function onStateChange(player, quest, sequence)
    local data = quest:GetData();

    -- Quest not accepted:  Set up actors to let you retrieve the item & initiate the log-in check at the Inn
    if (sequence == SEQ_ACCEPT) then
        quest:SetENpc(KOPURU_FUPURU, QFLAG_TALK);
        quest:SetENpc(VKOROLON, QFLAG_TALK);
        quest:SetENpc(MYTESYN, QFLAG_TALK);
        quest:SetENpc(BED_ULDAH, 5);
        quest:SetENpc(BED_GRIDANIA, 5);
        quest:SetENpc(BED_LIMSA, 5);
    -- Quest started
    elseif (sequence == SEQ_000) then
        quest:SetENpc(COFFER_AND_COFFIN_PUSH, QFLAG_PUSH, false, true, false, true);
    elseif (sequence == SEQ_005) then
        local bane1Flag = data:GetFlag(FLAG_SEQ005_BANE_1) and QFLAG_NONE or QFLAG_TALK;
        local bane2Flag = data:GetFlag(FLAG_SEQ005_BANE_2) and QFLAG_NONE or QFLAG_TALK;
        local bane3Flag = data:GetFlag(FLAG_SEQ005_BANE_3) and QFLAG_NONE or QFLAG_TALK;
        local bane4Flag = data:GetFlag(FLAG_SEQ005_BANE_4) and QFLAG_NONE or QFLAG_TALK;
        local bane5Flag = data:GetFlag(FLAG_SEQ005_BANE_5) and QFLAG_NONE or QFLAG_TALK;
    
        quest:SetENpc(COFFER_AND_COFFIN_PUSH, QFLAG_PUSH, false, true, false, true);
        quest:SetENpc(HILDIBRAND);
        quest:SetENpc(NASHU_MHAKARACCA);
        quest:SetENpc(ALRET);
        quest:SetENpc(BOMB_BANE_1, bane1Flag);
        quest:SetENpc(BOMB_BANE_2, bane2Flag);
        quest:SetENpc(BOMB_BANE_3, bane3Flag);
        quest:SetENpc(BOMB_BANE_4, bane4Flag);
        quest:SetENpc(BOMB_BANE_5, bane5Flag);
    -- Quest finished
    elseif (sequence == SEQ_010) then
        quest:SetENpc(HILDIBRAND);
        quest:SetENpc(NASHU_MHAKARACCA);    
        quest:SetENpc(ALRET, QFLAG_REWARD);
    end
end

function onTalk(player, quest, npc)
    local sequence = quest:getSequence();
    local classId = npc:GetActorClassId();

    if (sequence == SEQ_ACCEPT) then
        if (classId == KOPURU_FUPURU) then
            callClientFunction(player, "delegateEvent", player, quest, "processEventKOPURUStart");
        elseif (classId == VKOROLON) then 
            callClientFunction(player, "delegateEvent", player, quest, "processEventKOROLONStart");
        elseif (classId == MYTESYN) then
            callClientFunction(player, "delegateEvent", player, quest, "processEventMYTESYNStart");
        end
        giveDossierItem(player);
        player:SendGameMessage(player, GetWorldMaster(), 51149, MESSAGE_TYPE_SYSTEM, ITEM_HIDLIBRAND_DOSSIER);
        
    elseif (sequence == SEQ_005) then
        local data = quest:GetData();
        local incCounter = false;
    
        if (classId == HILDIBRAND) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_005_1");
        elseif (classId == NASHU_MHAKARACCA) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_005_2");    
        elseif (classId == ALRET) then
            if (sequence == SEQ_005) then
                callClientFunction(player, "delegateEvent", player, quest, "processEvent_005_3");
            end
        elseif (classId == BOMB_BANE_1) then
            if not (data:GetFlag(FLAG_SEQ005_BANE_1)) then
               incCounter = true;
               data:SetFlag(FLAG_SEQ005_BANE_1);
            end
        elseif (classId == BOMB_BANE_2) then
            if not (data:GetFlag(FLAG_SEQ005_BANE_2)) then
               incCounter = true;
               data:SetFlag(FLAG_SEQ005_BANE_2);
            end
        elseif (classId == BOMB_BANE_3) then
            if not (data:GetFlag(FLAG_SEQ005_BANE_3)) then
               incCounter = true;
               data:SetFlag(FLAG_SEQ005_BANE_3);
            end
        elseif (classId == BOMB_BANE_4) then
            if not (data:GetFlag(FLAG_SEQ005_BANE_4)) then
               incCounter = true;
               data:SetFlag(FLAG_SEQ005_BANE_4);
            end            
        elseif (classId == BOMB_BANE_5) then
            if not (data:GetFlag(FLAG_SEQ005_BANE_5)) then
               incCounter = true;
               data:SetFlag(FLAG_SEQ005_BANE_5);
            end        
        end
        
        if (incCounter == true) then
            counterAmount = data:IncCounter(COUNTER_BANE);
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_005_4", counterAmount, 5);
            wait(1);
            
            if (counterAmount >= 5) then
                attentionMessage(player, 25225, quest:GetQuestId()); -- "Seeing the Seers" objectives complete!
                quest:GetData():ClearData();
                quest:UpdateENPCs(); -- Band-aid for a QFLAG_TALK issue
                quest:StartSequence(SEQ_010);
            end
        end
    
    elseif (sequence == SEQ_010) then
        if (classId == HILDIBRAND) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_005_1");
        elseif (classId == NASHU_MHAKARACCA) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_005_2");    
        elseif (classId == ALRET) then  -- Finish the quest
                callClientFunction(player, "delegateEvent", player, quest, "processEvent_020");
                --TO-DO: Get the scaled EXP for this sqrwa figured out
                --TO-DO: Also confirm reward was issued before flagging quest as complete
                callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 1000, 1, 1, 9);
                GetWorldManager():WarpToPublicArea(player);
                player:CompleteQuest(quest);
        end
    end
    
    player:EndEvent()
	quest:UpdateENPCs();
end



function onPush(player, quest, npc)
    local sequence = quest:getSequence();
	local classId = npc.GetActorClassId();
	
    if (classId == COFFER_AND_COFFIN_PUSH) then
        if (sequence == SEQ_000) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_005");
            player:EndEvent();
            attentionMessage(player, 25246, ITEM_BOMB_BANE, 1);
            quest:StartSequence(SEQ_005);
        end

        GetWorldManager():WarpToPrivateArea(player, "PrivateAreaMasterPast", 5, -1732.891, 56.119, -307.285, -2.785);
        actor = player.CurrentArea:FindActorInZoneByUniqueID("etc5l3_nashu"); 
        actor:ChangeState(ACTORSTATE_SITTING_ONFLOOR); -- Band-aid to get her sitting.
    end
    player:EndEvent();
end

function getJournalInformation(player, quest)
    -- Bugged on the client's end and never shows?
	return 0, ITEM_BOMB_BANE;
end


function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
	local data = quest:GetData();
    local possibleMarkers = {};
    
    if (sequence == SEQ_000) then  
        return MRKR_COFFIN;
        
    elseif (sequence == SEQ_005) then
        local data = quest:GetData();
        if (not data:GetFlag(FLAG_SEQ005_BANE_1)) then table.insert(possibleMarkers, MRKR_BANE_1); end
        if (not data:GetFlag(FLAG_SEQ005_BANE_2)) then table.insert(possibleMarkers, MRKR_BANE_2); end
        if (not data:GetFlag(FLAG_SEQ005_BANE_3)) then table.insert(possibleMarkers, MRKR_BANE_3); end
        if (not data:GetFlag(FLAG_SEQ005_BANE_4)) then table.insert(possibleMarkers, MRKR_BANE_4); end
        if (not data:GetFlag(FLAG_SEQ005_BANE_5)) then table.insert(possibleMarkers, MRKR_BANE_5); end
        return unpack(possibleMarkers)
        
    elseif (sequence == SEQ_010) then
        return MRKR_ALRET;
    end
end


function giveDossierItem(player)

    local invCheck = player:getItemPackage(INVENTORY_NORMAL):addItem(ITEM_HIDLIBRAND_DOSSIER, 1, 1);
            
    if (invCheck == INV_ERROR_SUCCESS) then
        player:SendGameMessage(player, GetWorldMaster(), 25246, MESSAGE_TYPE_SYSTEM_ERROR, ITEM_HIDLIBRAND_DOSSIER, 1);
        return true;
    end
end