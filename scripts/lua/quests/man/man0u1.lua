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
MRKR_YAYATOKI           = 11001003;
MRKR_ADV_GUILD          = 11001004;

-- Quest Items
ITEM_VELODYNA_COSMOS = 0; -- Seq_000 : 2nd journal arg.    >=5 doesn't have.
ITEM_COLISEUM_PASS   = 0; -- Seq_015 : 3rd journal arg.    >=5 doesn't have

-- Quest Flags
FLAG_SEQ000     = 0; 
FLAG_SEQ010_TALK0 = 1;

function onStart(player, quest) 
    quest:StartSequence(SEQ_000);
    
    -- Immediately move to the Adventurer's Guild
    player:Warp(175);
end

function onStateChange(player, quest, state)
    if (state == QUEST_ACCEPTED) then
        -- Start the quest
        quest:StartSequence(SEQ_000);
    elseif (state == QUEST_COMPLETED) then
        -- End the quest
    end
end

function onTalk(player, quest, speaker)
    if (speaker == MOMODI) then
        if (quest:GetSequence() == SEQ_000) then
            -- Start the sequence
            quest:StartSequence(SEQ_005);
        elseif (quest:GetSequence() == SEQ_005) then
            -- Attune and start the next sequence
            player:Warp(175);
            quest:StartSequence(SEQ_010);
        elseif (quest:GetSequence() == SEQ_010) then
            -- Return to the guild
            player:Warp(175);
            quest:StartSequence(SEQ_012);
        elseif (quest:GetSequence() == SEQ_012) then
            -- Speak to Momodi
            quest:StartSequence(SEQ_015);
        elseif (quest:GetSequence() == SEQ_015) then
            -- Visiting guilds (GSM, GLD)
            quest:StartSequence(SEQ_045);
        elseif (quest:GetSequence() == SEQ_045) then
            -- Miner's Guild
            quest:StartSequence(SEQ_050);
        elseif (quest:GetSequence() == SEQ_050) then
            -- Miner's Guild Instance #1
            quest:StartSequence(SEQ_051);
        elseif (quest:GetSequence() == SEQ_051) then
            -- Emotes
            quest:StartSequence(SEQ_055);
        elseif (quest:GetSequence() == SEQ_055) then
            -- Miner's Guild Instance #2
            quest:CompleteQuest();
        end
    end
end

function onPush(player, quest, pushEvent)
    -- Remove debug message
    -- player:SendMessage(0x20, "", "Sequence: "..quest:GetSequence().." Class Id: "..player:GetClassId());
end

function getJournalMapMarkerList(player, quest)
    local possibleMarkers = {};
    
    if (quest:GetSequence() == SEQ_000) then
        table.insert(possibleMarkers, MRKR_MOMODI);
    elseif (quest:GetSequence() == SEQ_005) then
        table.insert(possibleMarkers, MRKR_CAMP_BLACK_BRUSH);
    elseif (quest:GetSequence() == SEQ_010) then
        if (not quest:GetFlag(FLAG_SEQ010_TALK0)) then
            table.insert(possibleMarkers, MRKR_YAYATOKI);
        else
            table.insert(possibleMarkers, MRKR_ADV_GUILD);
        end
    end
    
    return possibleMarkers;
end