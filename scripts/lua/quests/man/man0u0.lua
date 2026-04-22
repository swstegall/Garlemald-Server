require ("global")
require ("quest")

--[[

Quest Script

Name:   Flowers for All
Code:   Man0u0
Id:     110009
Prereq: None (Given on chara creation)
Notes: RURURAJI scripting handled via PopulaceChocoboLender.lua
TO-DO: Sequence 000 - Crowd NPCs.
       Sequence 010 - Adv. Guild NPCs


https://www.youtube.com/watch?v=XXGrSFrfYo4

]]

-- Sequence Numbers
SEQ_000 = 0;  -- On the Merchant Strip in Ul'dah; contains the basic tutorial.
SEQ_005 = 5;  -- Combat on the Sapphire Avenue Exchange
SEQ_010 = 10; -- Back on the Merchant Strip in Ul'dah

-- Actor Class Ids
ASCILIA                 = 1000042;
WARBURTON               = 1000186;
RURURAJI                = 1000840;
BIG_BELLIED_BARKER      = 1001490;
FRETFUL_FARMHAND        = 1001491;
DEBAUCHED_DEMONESS      = 1001492;
DAPPER_DAN              = 1001493;
LOUTISH_LAD             = 1001494;
GIL_DIGGING_MISTRESS    = 1001495;
TWITTERING_TOMBOY       = 1001496;
STOCKY_STRANGER         = 1001644;
EXIT_TRIGGER            = 1090372;
OPENING_STOPER_ULDAH    = 1090373;



KEEN_EYED_MERCHANT      = 1000401;
--MUMPISH_MIQOTE          = 1000992; -- Unused on this client version.  Calls processEvent020_6
HIGH_SPIRITED_FELLOW    = 1001042;
DISREPUTABLE_MIDLANDER  = 1001044;
LONG_LEGGED_LADY        = 1001112;
LARGE_LUNGED_LABORER    = 1001645;
TOOTH_GRINDING_TRAVELER = 1001646;
FULL_LIPPED_FILLE       = 1001647;
YAYATOKI                = 1500129;

BLOCKER                 = 1090372;
ULDAH_OPENING_EXIT      = 1099046;

-- Non-interactive NPCs
CROWD_HYUR_M            = 1001114;
CROWD_HYUR_F            = 1001115;
CROWD_ELEZEN_M          = 1001116;
CROWD_ELEZEN_F          = 1001117;
CROWD_LALAFELL_M        = 1001118;
CROWD_LALAFELL_F        = 1001119;
CROWD_MIQOTE            = 1001120;
CROWD_ROEGADYN          = 1001121;
GUILD_KIORA             = 1000780;
GUILD_OPONDHAO          = 1000781;
GUILD_BERTRAM           = 1000782;
GUILD_MINERVA           = 1000783;
GUILD_ZOENGTERBIN       = 1000784;
GUILD_STYRMOEYA         = 1000785;
GUILD_YHAH_AMARIYO      = 1000786;
GUILD_HILDIE            = 1000787;
GUILD_LETTICE           = 1000788;
GUILD_TYON              = 1000789;
GUILD_OTOPA_POTTOPA     = 1000864;
GUILD_THAISIE           = 1000865;
GUILD_SESEBARU          = 1001182;
GUILD_TOTONAWA          = 1001371;
GUILD_EUSTACE           = 1001372;


-- Quest Markers
MRKR_YAYATOKI               = 11000901;
MRKR_ASCILIA                = 11000902;
MRKR_FRETFUL_FARMHAND       = 11000903;
MRKR_GIL_DIGGING_MISTRESS   = 11000904;
MRKR_COMBAT_TUTORIAL        = 11000905;
MRKR_ADV_GUILD              = 11000906;


-- Quest Flags
FLAG_SEQ000_MINITUT0    = 0; -- PushEvent ASCILIA
FLAG_SEQ000_MINITUT1    = 1; -- TalkEvent ASCILIA
FLAG_SEQ000_MINITUT2    = 2; -- TalkEvent FRETFUL_FARMHAND
FLAG_SEQ000_MINITUT3    = 3; -- TalkEvent GIL_DIGGING_MISTRESS

FLAG_SEQ010_TALK0       = 0; -- TalkEvent YAYATOKI


function onStart(player, quest)
    quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)
	local data = quest:GetData();

    if (sequence == SEQ_000) then
        -- Setup states incase we loaded in.
        local asciliaCanPush = not data:GetFlag(FLAG_SEQ000_MINITUT0);
        local asciliaFlag = data:GetFlag(FLAG_SEQ000_MINITUT1) and QFLAG_NONE or QFLAG_TALK;
        local fretfulfarmhandFlag = data:GetFlag(FLAG_SEQ000_MINITUT2) and QFLAG_NONE or QFLAG_TALK;
        local gildiggingmistressFlag = data:GetFlag(FLAG_SEQ000_MINITUT3) and QFLAG_NONE or QFLAG_TALK;

        local exitFlag = data:GetFlags() == 0xF and QFLAG_PUSH or QFLAG_NONE;

        if (asciliaCanPush) then
            fretfulfarmhandFlag = QFLAG_NONE;
            gildiggingmistressFlag = QFLAG_NONE;
        end

        --SetENpc(classId, byte flagType=0,isTalkEnabled, isPushEnabled, isEmoteEnabled, isSpawned)
        quest:SetENpc(ASCILIA, asciliaFlag, true, asciliaCanPush);
        quest:SetENpc(WARBURTON);
        quest:SetENpc(RURURAJI);
        quest:SetENpc(BIG_BELLIED_BARKER);
        quest:SetENpc(FRETFUL_FARMHAND, fretfulfarmhandFlag);
        quest:SetENpc(DEBAUCHED_DEMONESS);
        quest:SetENpc(DAPPER_DAN);
        quest:SetENpc(LOUTISH_LAD);
        quest:SetENpc(GIL_DIGGING_MISTRESS, gildiggingmistressFlag);
        quest:SetENpc(TWITTERING_TOMBOY);
        quest:SetENpc(STOCKY_STRANGER);
        quest:SetENpc(EXIT_TRIGGER, exitFlag, false, true);
        quest:SetENpc(OPENING_STOPER_ULDAH, QFLAG_NONE, false, false, true);

    elseif (sequence == SEQ_010) then
        local yayatokiFlag = data:GetFlag(FLAG_SEQ010_TALK0) and QFLAG_NONE or QFLAG_TALK;
        local uldahopeningexitFlag = QFLAG_PUSH;
        quest:SetENpc(KEEN_EYED_MERCHANT);
        quest:SetENpc(HIGH_SPIRITED_FELLOW);
        quest:SetENpc(DISREPUTABLE_MIDLANDER);
        quest:SetENpc(LONG_LEGGED_LADY);
        quest:SetENpc(LARGE_LUNGED_LABORER);
        quest:SetENpc(TOOTH_GRINDING_TRAVELER);
        quest:SetENpc(FULL_LIPPED_FILLE);
        quest:SetENpc(YAYATOKI, yayatokiFlag);
        quest:SetENpc(BLOCKER, QFLAG_NONE, false, true);
        quest:SetENpc(ULDAH_OPENING_EXIT, uldahopeningexitFlag, false, true);
    end
end

function onTalk(player, quest, npc)
    local sequence = quest:getSequence();
    local classId = npc:GetActorClassId();

    if (sequence == SEQ_000) then
        seq000_onTalk(player, quest, npc, classId);
    elseif (sequence == SEQ_010) then
        seq010_onTalk(player, quest, npc, classId);
    end
    quest:UpdateENPCs();
end

function onPush(player, quest, npc)
    local sequence = quest:getSequence();
    local classId = npc:GetActorClassId();
	local data = quest:GetData();

    if (sequence == SEQ_000) then
        if (classId == ASCILIA) then
           callClientFunction(player, "delegateEvent", player, quest, "processTtrNomal002");
           player:EndEvent();
        elseif (classId == EXIT_TRIGGER) then
            if (data:GetFlags() == 0xF) then
                doExitTrigger(player, quest, npc);
                return;
            else
                callClientFunction(player, "delegateEvent", player, quest, "processTtrBlkNml001");
                GetWorldManager():DoPlayerMoveInZone(player, -22, 196, 87, 2.4, 0x11)
                player:EndEvent();
            end
        end
    elseif (sequence == SEQ_010) then
        if (classId == BLOCKER) then

            posz = player:GetPos()[3];

            if (posz >= 71 and posz <= 95) then
                callClientFunction(player, "delegateEvent", player, quest, "processTtrBlkNml002");
                GetWorldManager():DoPlayerMoveInZone(player, -22.81, 196, 87.82, 2.98, 0x11);
            else
                callClientFunction(player, "delegateEvent", player, quest, "processTtrBlkNml003");
                GetWorldManager():DoPlayerMoveInZone(player, -0.3, 196, 116, -2.7, 0x11);
            end
        elseif (classId == ULDAH_OPENING_EXIT) then
            player:ReplaceQuest(quest, "Man0u1")
            return;
        end
    end
    quest:UpdateENPCs();
end

function onNotice(player, quest, target)
    callClientFunction(player, "delegateEvent", player, quest, "processTtrNomal001withHQ");
    player:EndEvent();
    quest:UpdateENPCs();
end

function seq000_onTalk(player, quest, npc, classId)
	local data = quest:GetData();

    if (classId == ASCILIA) then
        if (not data:GetFlag(FLAG_SEQ000_MINITUT0)) then -- If Talk tutorial
            callClientFunction(player, "delegateEvent", player, quest, "processTtrNomal003");
            quest:GetData():SetFlag(FLAG_SEQ000_MINITUT0); -- Used to disable her PushEvent / Allow for her next TalkEvent
        else
            callClientFunction(player, "delegateEvent", player, quest, "processTtrMini001");
            quest:GetData():SetFlag(FLAG_SEQ000_MINITUT1); -- Ascilia has now been talked to.
        end

    elseif (classId == FRETFUL_FARMHAND) then
        if (not data:GetFlag(FLAG_SEQ000_MINITUT2)) then
            callClientFunction(player, "delegateEvent", player, quest, "processTtrMini002_first");
            data:SetFlag(FLAG_SEQ000_MINITUT2);
        else
            callClientFunction(player, "delegateEvent", player, quest, "processTtrMini002");
        end

    elseif (classId == GIL_DIGGING_MISTRESS) then
        if (not data:GetFlag(FLAG_SEQ000_MINITUT3)) then
            callClientFunction(player, "delegateEvent", player, quest, "processTtrMini003_first");
            data:SetFlag(FLAG_SEQ000_MINITUT3);
        else
            callClientFunction(player, "delegateEvent", player, quest, "processTtrMini003");
        end

    elseif (classId == WARBURTON) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_3");
    elseif (classId == RURURAJI) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_13");
    elseif (classId == BIG_BELLIED_BARKER) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_6");
    elseif (classId == DEBAUCHED_DEMONESS) then
         callClientFunction(player, "delegateEvent", player, quest, "processEvent000_8");
    elseif (classId == DAPPER_DAN) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_9");
    elseif (classId == LOUTISH_LAD) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_10");
    elseif (classId == TWITTERING_TOMBOY) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_12");
    elseif (classId == STOCKY_STRANGER) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_6_2");
    end

    player:EndEvent();
end

function seq010_onTalk(player, quest, npc, classId)
    if (classId == KEEN_EYED_MERCHANT) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent020_2");
    elseif (classId == HIGH_SPIRITED_FELLOW) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent020_3");
    elseif (classId == DISREPUTABLE_MIDLANDER) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent020_4");
    elseif (classId == LONG_LEGGED_LADY) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_14");
    elseif (classId == LARGE_LUNGED_LABORER) then
        callClientFunction(player, "delegateEvent", player, quest, "processEtc003");
    elseif (classId == TOOTH_GRINDING_TRAVELER) then
        callClientFunction(player, "delegateEvent", player, quest, "processEtc001");
    elseif (classId == FULL_LIPPED_FILLE) then
        callClientFunction(player, "delegateEvent", player, quest, "processEtc002");
    elseif (classId == YAYATOKI) then
        if (not quest:GetData():GetFlag(FLAG_SEQ010_TALK0)) then
            callClientFunction(player, "delegateEvent", player, quest, "processEvent020_8");
            quest:GetData():SetFlag(FLAG_SEQ010_TALK0);
        else
            callClientFunction(player, "delegateEvent", player, quest, "processEvent020_8");
        end
    end

    player:EndEvent();
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
	local data = quest:GetData();
    local possibleMarkers = {};

    if (sequence == SEQ_000) then
        if (data:GetFlag(FLAG_SEQ000_MINITUT0)) then
            if (not data:GetFlag(FLAG_SEQ000_MINITUT1)) then table.insert(possibleMarkers, MRKR_ASCILIA); end
            if (not data:GetFlag(FLAG_SEQ000_MINITUT2)) then table.insert(possibleMarkers, MRKR_FRETFUL_FARMHAND); end
            if (not data:GetFlag(FLAG_SEQ000_MINITUT3)) then table.insert(possibleMarkers, MRKR_GIL_DIGGING_MISTRESS); end
        end

    elseif (sequence == SEQ_010) then
        if (not data:GetFlag(FLAG_SEQ010_TALK0)) then
            table.insert(possibleMarkers, MRKR_YAYATOKI)
        end
            table.insert(possibleMarkers, MRKR_ADV_GUILD);
    end

    return unpack(possibleMarkers)
end




function doExitTrigger(player, quest, npc)	
    quest:GetData():ClearData();
    quest:StartSequence(SEQ_005);
    contentArea = player.CurrentArea:CreateContentArea(player, "/Area/PrivateArea/Content/PrivateAreaMasterSimpleContent", "man0u01", "SimpleContent30079", "Quest/QuestDirectorMan0u001");

    if (contentArea == nil) then
        return;
    end

    director = contentArea:GetContentDirector();
    player:AddDirector(director);
    director:StartDirector(false);

    player:KickEvent(director, "noticeEvent", true);
    player:SetLoginDirector(director);

    GetWorldManager():DoZoneChangeContent(player, contentArea, -24.34, 192, 34.22, 0.78, 16);
    return;
end


