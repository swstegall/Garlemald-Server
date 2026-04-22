require ("global")
require ("quest")

--[[

Quest Script

Name:   Private Eyes 
Code:   Etc5l1
Id:     110839
Prereq: Level 15.  Man5g1 (In Plain Sight) complete.  [110829]
Notes: 

]]

-- Sequence Numbers
SEQ_000 = 0;  -- Talk to Bertrand
SEQ_010 = 10; -- Head to the meeting point.

-- Actor Class Ids
OTOPA_POTTOPA           = 1000864;
MYTESYN                 = 1000167;
MIZZENMAST_BED          = 1200378;
BERTRAND                = 1001903; -- 1060004 also a valid ID, but doesn't look like the NPC appeared anywhere else?
ABRAHAM                 = 1002066; 
PRIVATE_AREA_ENTRANCE   = 1090087; -- Check that this ID is free to use before merge
PRIVATE_AREA_EXIT       = 1290002;
CUTSCENE_PUSH_TRIGGER   = 1090088; -- Check that this ID is free to use before merge

-- Prop Actor Ids (for documentation sake)
BRONZE_CHEST            = 1080056; -- bgObj 20923 w/ body 1024
GLASS_DRINK             = 1080057; -- bgObj 20901 w/ body 26624
RECTANGULAR_BOX         = 1080058; -- bgObj 20951 w/ body 1024

-- Quest Items
ITEM_WANTED_GAUWYN      = 10011243;

-- Quest Markers
MRKR_CAVE               = 11072101;
MRKR_BERTRAND           = 11072102;
MRKR_CUTSCENE           = 11072103;



function onStart(player, quest)
    quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end


function onStateChange(player, quest, sequence)

    if (sequence == SEQ_ACCEPT) then
        local hasQuestItem = player:GetItemPackage(INVENTORY_NORMAL):HasItem(ITEM_WANTED_GAUWYN);
        local otopaFlag = 0;
        
        if (hasQuestItem == false) then 
            otopaFlag = 2; 
        end
        quest:SetENpc(OTOPA_POTTOPA, otopaFlag);
        quest:SetENpc(MYTESYN, QFLAG_TALK); -- Assuming this functions the same as it did in Etc5g1
        quest:SetENpc(MIZZENMAST_BED, 5);
        
    elseif (sequence == SEQ_000) then
        quest:SetENpc(MYTESYN);
        quest:SetENpc(PRIVATE_AREA_ENTRANCE, QFLAG_PUSH, false, true, false, true);
        quest:SetENpc(BERTRAND, QFLAG_TALK);
        quest:SetENpc(ABRAHAM);
    elseif (sequence == SEQ_010) then 
        quest:SetENpc(CUTSCENE_PUSH_TRIGGER, QFLAG_PUSH,false, true, false, true);
    end
end

function onTalk(player, quest, npc)
    local sequence = quest:getSequence();
    local classId = npc:GetActorClassId();
    
    if (sequence == SEQ_ACCEPT) then
        -- Am assuming this all functions the same as it did in Etc5g1
        if (classId == OTOPA_POTTOPA) then
            local hasQuestItem = player:GetItemPackage(INVENTORY_NORMAL):HasItem(ITEM_WANTED_GAUWYN);
            
            if (not hasQuestItem) then
                callClientFunction(player, "delegateEvent", player, quest, "processEventOTOPAPOTTOPAStart");
                giveWantedItem(player);
                npc:SetQuestGraphic(player, QFLAG_NONE);
            else
                callClientFunction(player, "delegateEvent", player, quest, "processEventOTOPAPOTTOPAStart_2");
            end 
            player:SendGameMessage(player, GetWorldMaster(), 51148, MESSAGE_TYPE_SYSTEM, 10011243, 1070);  -- Log out in Mizzenmast Inn w/ item.
        elseif (classId == MYTESYN) then
            player:SendGameMessage(player, GetWorldMaster(), 51148, MESSAGE_TYPE_SYSTEM, 10011243, 1070); 
        end
        
    elseif (sequence == SEQ_000) then
        if (classId == MYTESYN) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_000_1"); 
        elseif (classId == BERTRAND) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_010");
            quest:UpdateENPCs(); -- Band-aid for a QFLAG_TALK issue
            quest:StartSequence(SEQ_010);
            GetWorldManager():WarpToPublicArea(player);
        elseif (classId == ABRAHAM) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent_010_1"); 
        end
    end
    
    player:EndEvent()
	quest:UpdateENPCs();
end



function onPush(player, quest, npc)
	local classId = npc.GetActorClassId();
	
    
    if (classId == PRIVATE_AREA_ENTRANCE) then
        choice = callClientFunction(player, "delegateEvent", player, quest, "instanceAreaJoinAskInBasaClass");
        if (choice == 1) then
            GetWorldManager():WarpToPrivateArea(player, "PrivateAreaMasterPast", 5, -220.948, 16.603, -92.863, -2.090);
        end
        
    elseif (classId == CUTSCENE_PUSH_TRIGGER) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent_020"); 
        callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 500, 1, 1);
        player:CompleteQuest(quest);
    end
    player:EndEvent();
end



function getJournalInformation(player, quest)
end


function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();

    if (sequence == SEQ_000) then  
        return MRKR_CAVE;
        -- TO-DO: Check for private area and use MRKR_BERTRAND in place of this
    elseif (sequence == SEQ_010) then
        return MRKR_CUTSCENE;
    end
end


function giveWantedItem(player)

    local invCheck = player:getItemPackage(INVENTORY_NORMAL):addItem(ITEM_WANTED_GAUWYN, 1, 1);
            
    if (invCheck == INV_ERROR_FULL) then
        -- Your inventory is full.
        player:SendGameMessage(player, GetWorldMaster(), 60022, MESSAGE_TYPE_SYSTEM_ERROR);
    elseif (invCheck == INV_ERROR_ALREADY_HAS_UNIQUE) then
        -- You cannot have more than one <itemId> <quality> in your possession at any given time.
        player:SendGameMessage(player, GetWorldMaster(), 40279, MESSAGE_TYPE_SYSTEM_ERROR, ITEM_WANTED_GAUWYN, 1);
    elseif (invCheck == INV_ERROR_SYSTEM_ERROR) then
        player:SendMessage(MESSAGE_TYPE_SYSTEM, "", "[DEBUG] Server Error on adding item.");
    elseif (invCheck == INV_ERROR_SUCCESS) then
        player:SendGameMessage(player, GetWorldMaster(), 25246, MESSAGE_TYPE_SYSTEM_ERROR, ITEM_WANTED_GAUWYN, 1);
    end
end