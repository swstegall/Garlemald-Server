require ("global")
require ("quest")

--[[

Quest Script

Name:   Court in the Sands 
Code:   Man0u1
Id:     110010
Prereq: Flowers for All (Man0u0 - 110009)
Notes:


Vid refs - 
https://www.youtube.com/watch?v=WNRLrwZ3BJY&t=284s
https://www.youtube.com/watch?v=eZgcq-FMpfw&t=504s

Coliseum fight - https://www.youtube.com/watch?v=Jcv9I2Bk46w

A LOT - https://www.youtube.com/watch?v=gySHO1Be9OM

]]


--[[ 

Phase:
        
45  (Miner's Guild)
        Linette   processEvent050 (initial CS)
        Change phase to 50 after interaction
        
50  (Miner's Guild Instance #1)
        Name                    DisplayName             ActorClass                          Event
        Linette                 1100016                 1000861                             processEvent050_2
        Corguevais              1200025                 1000043/1001054                     processEvent050_11
        Nittma Guttma           1400127                 1001286                             processEvent050_10
        Nortmoen                1600127                 1600042
        F'lhaminn               1900054                 1000038/1000842/1001514/2290008     processEvent051_1
        Tyago Moui              1900130                 1001203                             processEvent050_12
        Shilgen                 2200216                 1000637
        Muscular Miner          4000202                 1000690/1700013                     processEvent050_7
        Close-fisted Woman      4000366                 1000981                             processEvent050_8
        Astonished Adventurer   4000377                 1000895                             processEvent050_9
        Manic Miner             4000444                 1001283                             processEvent050_13
        Maddened Miner          4000445                 1001284                             processEvent050_14
        Maudlin Miner           4000446                 1001287                             processEvent050_3
        Mocking Miner           4000447                 1001288                             processEvent050_4
        Monitoring Miner        4000448                 1001289                             processEvent050_5
        Displeased Dancer       4000449                 1001290                             processEvent050_6
        
        Emotes 103, 108, 121, 125, 140, 135  in that order @ F'lhaminn, then change phase to 51
51
        Emotes 108 @ Maddened Miner
        Emotes 135, 103, 121 @ Manic Miner   
        Check both for clear state after each interaction and change phase to 55
        
55  (Miner's Guild Instance #2)

--]]

-- Sequence Numbers
SEQ_000 = 0; -- Ul'dah Adventurer's Guild
SEQ_005 = 5; -- Run to Camp Black Brush & Attune
SEQ_010 = 10; -- Return to the Guild
SEQ_012 = 12; -- Speak to Momodi
SEQ_015 = 15; -- Visiting guilds (GSM, GLD)
SEQ_045 = 45;
SEQ_050 = 50;
SEQ_057 = 57;
SEQ_058 = 58;
SEQ_060 = 60;
SEQ_065 = 65;
SEQ_070 = 70;
SEQ_075 = 75;
SEQ_080 = 80;
SEQ_085 = 85;
SEQ_090 = 90;
SEQ_095 = 95;
SEQ_100 = 100;
SEQ_105 = 105;
SEQ_110 = 110;

-- Actor Class Ids
OVERCOMPETITIVE_ADVENTURER  = 1000807;
MOMODI                      = 1000841;
OTOPA_POTTOPA               = 1000864;
UNDAUNTED_ADVENTURER        = 1000936;
GREEDY_MERCHANT             = 1000937;
LIONHEARTED_ADVENTURER      = 1000938;
SPRY_SALESMAN               = 1000939;

UPBEAT_ADVENTURER           = 1000940;
SEEMINGLY_CALM_ADVENTURER   = 1000941;
UNKNOWN1 = 0;
UNKNOWN2 = 0;

THANCRED = 1000948; -- 1000010


-- Quest Markers
MRKR_MOMODI             = 11001001;
MRKR_CAMP_BLACK_BRUSH   = 11001002;

-- Quest Items
ITEM_VELODYNA_COSMOS = 0; -- Seq_000 : 2nd journal arg.    >=5 doesn't have.
ITEM_COLISEUM_PASS   = 0; -- Seq_015 : 3rd journal arg.    >=5 doesn't have

-- Quest Flags
FLAG_SEQ000     = 0; 

function onStart(player, quest) 
    quest:StartSequence(SEQ_000);
    
    -- Immediately move to the Adventurer's Guild private area
	callClientFunction(player, "delegateEvent", player, quest, "processEventMomodiStart");
    GetWorldManager():DoZoneChange(player, 175, "PrivateAreaMasterPast", 4, 15, -75.242, 195.009, 74.572, -0.046);	
    player:SendGameMessage(quest, 329, 0x20);
	player:SendGameMessage(quest, 330, 0x20);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)

    if (sequence == SEQ_000) then
        -- Setup states incase we loaded in.

        --SetENpc(classId, byte flagType=0,isTalkEnabled, isPushEnabled, isEmoteEnabled, isSpawned)
        quest:SetENpc(MOMODI, QFLAG_TALK);
        quest:SetENpc(OTOPA_POTTOPA);

    elseif (sequence == SEQ_005) then 
        quest:SetENpc(MOMODI);
    end
end

function onTalk(player, quest, npc)
    local sequence = quest:getSequence();
    local classId = npc:GetActorClassId();
    
    if (sequence == SEQ_000) then
        seq000_onTalk(player, quest, npc, classId);
    elseif (sequence == SEQ_005) then
        seq005_onTalk(player, quest, npc, classId);     
    end
	quest:UpdateENPCs();
end

function onPush(player, quest, npc)

    local sequence = quest:getSequence();
    local classId = npc:GetActorClassId();  
    player:SendMessage(0x20, "", "Sequence: "..sequence.." Class Id: "..classId);
    if (sequence == SEQ_000) then
    
    elseif (sequence == SEQ_010) then
   
    end
	quest:UpdateENPCs();
end


function onNotice(player, quest, target)
    callClientFunction(player, "delegateEvent", player, quest, "processEvent000_1"); -- Describes what an Instance is
    player:EndEvent();
	quest:UpdateENPCs();
end

function seq000_onTalk(player, quest, npc, classId)
    
    if (classId == MOMODI) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
        player:EndEvent();
        quest:StartSequence(SEQ_005);
        GetWorldManager():DoZoneChange(player, 175, nil, 0, 15, player.positionX, player.positionY, player.positionZ, player.rotation);
        return;
    elseif (classId == UNDAUNTED_ADVENTURER) then
        callClientFunction (player, "delegateEvent", player, quest, "processEvent000_2");
    elseif (classId == GREEDY_MERCHANT) then
        callClientFunction (player, "delegateEvent", player, quest, "processEvent000_3");
    elseif (classId == SPRY_SALESMAN) then
        callClientFunction (player, "delegateEvent", player, quest, "processEvent000_4");
    elseif (classId == LIONHEARTED_ADVENTURER) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_5");
    elseif (classId == UPBEAT_ADVENTURER) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_6");
    elseif (classId == SEEMINGLY_CALM_ADVENTURER) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_7");
    elseif (classId == OVERCOMPETITIVE_ADVENTURER) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_8");        
    elseif (classId == OTOPA_POTTOPA) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_9");
    elseif (classId == THANCRED) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_10");        
    end

    player:EndEvent();
end

function seq005_onTalk(player, quest, npc, classId) 
    if (classId == MOMODI) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent010_2");
    end

    player:EndEvent();
end


function getJournalInformation(player, quest)
	return 0, ITEM_VELODYNA_COSMOS, ITEM_COLISEUM_PASS;
end


function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    local possibleMarkers = {};

    if (sequence == SEQ_000) then
        table.insert(possibleMarkers, MRKR_MOMODI);
    elseif (sequence == SEQ_010) then
        if (not quest:GetFlag(FLAG_SEQ010_TALK0)) then 
            table.insert(possibleMarkers, MRKR_YAYATOKI)
        else
            table.insert(possibleMarkers, MRKR_ADV_GUILD);
        end
    end

    return unpack(possibleMarkers)
end


