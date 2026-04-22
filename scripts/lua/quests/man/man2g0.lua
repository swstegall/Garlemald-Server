require ("global")
require ("quest")

--[[

Quest Script

Name: 	Beckon of the Elementals
Code: 	Man2g0
Id: 	110008
Prereq: Whispers in the Wood (Man1g0 - 110007), Level 8

]]

-- Sequence Numbers
SEQ_000	= 0;  	-- Visit the ARC guild and speak to Nonolato.
SEQ_003	= 3;  	-- Echo instance just before fight.
SEQ_004	= 4;  	-- Battle elemental duty.
SEQ_005	= 5;  	-- Return to CRP guild.
SEQ_010	= 10;  	-- Talk to Miounne on the LS.
SEQ_015	= 15;	-- Go to CNJ guild.
SEQ_020	= 20;	-- Enter the CNJ guild in echo.
SEQ_025	= 25;	-- Talk to Fye in CNJ echo.
SEQ_030	= 30;	-- Talk to O-App-Pesi in CNJ echo.
SEQ_035	= 35;	-- Head to Mih Khetto Amphitheatre
SEQ_040	= 40;	-- Amphitheatre echo, talk to Fye.
SEQ_045	= 45;	-- Weird ass echo with Yda/Papalymo.
SEQ_050	= 50;	-- Unused? Khrimm burning the tree.
SEQ_055	= 55;	-- Unused? Meteorshower and collapsing.
SEQ_060	= 60;	-- Unused? Post quest text.

-- Actor Class Ids
MIOUNNE 					= 1000230;

-- ARC Guild/Battle/Post Battle
NONOLATO					= 1000463;
ANAIDJAA					= 1000465;

-- Outside CNJ Guild
WISE_LOOKING_CONJURER		= 1000743;
DISQUIETED_LANCER			= 1000744;
ENTHUSIASTIC_ARCHER			= 1000745;
EMBITTERED_ARCHER			= 1000746;
DISCONCERTED_CONJURER		= 1000747;
CNJ_TRIGGER					= 0;
OUTSIDE_ECHO_TRIGGER		= 0;
CNJ_BRIDGE_TRIGGER			= 1090200;

-- CNJ Guild
FYE							= 1000014;
O_APP_PESI					= 1000033;
SOILEINE					= 1000234;
SILKY_HAIRED_CONJURER		= 1000748;
ENIGMATIC_CONJURER			= 1000749;

-- Amphitheatre
YDA							= 1000009;
PAPALYMO					= 1000010;
TKEBBE						= 1000015;
NGOLBB						= 1000016;
FARRIMOND					= 1000017;
BURCHARD					= 1000018;
URSBAEN						= 1000022;
HANDELOUP					= 1000023;
GRINNAUX					= 1000024;
FUFUCHA						= 1000237;
WISPILY_WHISKERED_WOODWORKER = 1000562;
NIALL						= 1000754;
RESPECTABLE_ROEGADYN		= 1000755;
AGING_ELEZEN				= 1000756;
URBANE_ELEZEN				= 1000757;
SUGARSTRUNG_SCHOOLGIRL		= 1000824;
UNDERPRIVILEGED_URCHIN		= 1000825;
SQUEALING_SPRAT				= 1000826;
DESERTED_DAUGHTER			= 1000827;
TROUBLESOME_TOMBOY			= 1000828;
OVERANIMATED_HYUR			= 1001485;
GOOD_NATURED_GOODWIFE		= 1001486;
FASTIDIOUS_FELLOW			= 1001487;
MERRY_OLD_MATRON			= 1001488;
WELL_GROOMED_WOMAN			= 1001489;

AMPHITHEATRE_TRIGGER		= 1090279;
FST_ECHO_TRIGGER			= 0;

-- Msg packs for the Npc LS
NPCLS_MSGS = {
	{65, 66, 67}, 			-- SEQ_010
	{112, 113, 114, 115}, 	-- SEQ_070
};

-- Quest Markers
MRKR_STEP1				= 11000801;
MRKR_STEP2				= 11000802;
MRKR_STEP3				= 11000803;
MRKR_STEP4				= 11000804;
MRKR_STEP5				= 11000805;
MRKR_STEP6				= 11000806;
MRKR_STEP7				= 11000807;
MRKR_STEP8				= 11000808;
MRKR_STEP9				= 11000809;
MRKR_STEP10				= 11000810;
MRKR_STEP11				= 11000811;
MRKR_STEP12				= 11000812;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)
	local data = quest:GetData();
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(MIOUNNE, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
		quest:SetENpc(NONOLATO, QFLAG_TALK);
		quest:SetENpc(MIOUNNE);
	elseif (sequence == SEQ_003) then
		quest:SetENpc(O_APP_PESI, QFLAG_TALK);
		quest:SetENpc(NONOLATO);
	elseif (sequence == SEQ_005) then
		quest:SetENpc(ANAIDJAA, QFLAG_TALK);
	elseif (sequence == SEQ_015) then
		quest:SetENpc(CNJ_TRIGGER, QFLAG_PUSH, false, true);
	elseif (sequence == SEQ_020) then
		quest:SetENpc(OUTSIDE_ECHO_TRIGGER, QFLAG_PUSH, false, true);
		quest:SetENpc(CNJ_BRIDGE_TRIGGER);
		quest:SetENpc(WISE_LOOKING_CONJURER);
		quest:SetENpc(DISQUIETED_LANCER);
		quest:SetENpc(ENTHUSIASTIC_ARCHER);
		quest:SetENpc(EMBITTERED_ARCHER);
		quest:SetENpc(DISCONCERTED_CONJURER);
		quest:SetENpc(SILKY_HAIRED_CONJURER);
		quest:SetENpc(SOILEINE);
		quest:SetENpc(ENIGMATIC_CONJURER);
	elseif (sequence == SEQ_025) then
		quest:SetENpc(FYE, QFLAG_TALK);
		quest:SetENpc(O_APP_PESI);
		quest:SetENpc(SOILEINE);
		quest:SetENpc(ENIGMATIC_CONJURER);
		quest:SetENpc(SILKY_HAIRED_CONJURER);
		quest:SetENpc(DISCONCERTED_CONJURER);
	elseif (sequence == SEQ_030) then
		quest:SetENpc(O_APP_PESI, QFLAG_TALK);
		quest:SetENpc(FYE);
		quest:SetENpc(SOILEINE);
		quest:SetENpc(ENIGMATIC_CONJURER);
		quest:SetENpc(SILKY_HAIRED_CONJURER);
		quest:SetENpc(DISCONCERTED_CONJURER);
	elseif (sequence == SEQ_035) then
		quest:SetENpc(AMPHITHEATRE_TRIGGER, QFLAG_PUSH, false, true);
	elseif (sequence == SEQ_040) then
		quest:SetENpc(FYE, QFLAG_TALK);
		quest:SetENpc(YDA);
		quest:SetENpc(PAPALYMO);
		quest:SetENpc(TKEBBE);
		quest:SetENpc(NGOLBB);
		quest:SetENpc(FARRIMOND);
		quest:SetENpc(BURCHARD);
		quest:SetENpc(URSBAEN);
		quest:SetENpc(HANDELOUP);
		quest:SetENpc(GRINNAUX);
		quest:SetENpc(FUFUCHA);
		quest:SetENpc(WISPILY_WHISKERED_WOODWORKER);
		quest:SetENpc(NIALL);
		quest:SetENpc(RESPECTABLE_ROEGADYN);
		quest:SetENpc(AGING_ELEZEN);
		quest:SetENpc(URBANE_ELEZEN);
		quest:SetENpc(SUGARSTRUNG_SCHOOLGIRL);
		quest:SetENpc(UNDERPRIVILEGED_URCHIN);
		quest:SetENpc(SQUEALING_SPRAT);
		quest:SetENpc(DESERTED_DAUGHTER);
		quest:SetENpc(TROUBLESOME_TOMBOY);
		quest:SetENpc(OVERANIMATED_HYUR);
		quest:SetENpc(GOOD_NATURED_GOODWIFE);
		quest:SetENpc(FASTIDIOUS_FELLOW);
		quest:SetENpc(MERRY_OLD_MATRON);
		quest:SetENpc(WELL_GROOMED_WOMAN);
	elseif (sequence == SEQ_045) then
		quest:SetENpc(FST_ECHO_TRIGGER, QFLAG_PUSH, false, true);
		quest:SetENpc(YDA);
		quest:SetENpc(PAPALYMO);
	end	
end

function onTalk(player, quest, npc)
	local sequence = quest:getSequence();
	local classId = npc:GetActorClassId();

	if (sequence == SEQ_ACCEPT) then
		if (classId == MIOUNNE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventMiounneStart");
			player:EndEvent();
			player:AcceptQuest(quest, true);
			return;
		end
	elseif (sequence == SEQ_000) then
		if (classId == NONOLATO) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent007");
			quest:StartSequence(SEQ_003);
			player:EndEvent();
			quest:UpdateENPCs();
			GetWorldManager():DoZoneChange(player, 206, "PrivateAreaMasterPast", 0, 0, 1832.243, 16.352, 1834.965, 1.584);
			return;
		elseif (classId == MIOUNNE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005_2");
		end
	elseif (sequence == SEQ_003 or sequence == SEQ_004) then
		if (classId == O_APP_PESI) then
			if (sequence == SEQ_003) then
				if (callClientFunction(player, "delegateEvent", player, quest, "processEvent007_2") == 1) then
					startDuty(player, quest);
				end
			else
				if (callClientFunction(player, "delegateEvent", player, quest, "processEvent007_2_2") == 1) then
					startDuty(player, quest);
				end
			end
			return;
		elseif (classId == NONOLATO) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent007_3");
		end
	elseif (sequence == SEQ_005) then
		if (classId == ANAIDJAA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020");
			quest:NewNpcLsMsg(1);
			quest:StartSequence(SEQ_010);
		end
	elseif (sequence == SEQ_015) then
		if (classId == MIOUNNE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020_2");
		--elseif (classId == ZEZEKUTA) then
		--	callClientFunction(player, "delegateEvent", player, quest, "processEvent020_3");
		end
	elseif (sequence == SEQ_020) then
		if (classId == SOILEINE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030_2");
		elseif (classId == DISQUIETED_LANCER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030_3");
		elseif (classId == DISCONCERTED_CONJURER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030_4");
		elseif (classId == WISE_LOOKING_CONJURER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030_5");
		elseif (classId == ENIGMATIC_CONJURER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030_6");
		elseif (classId == SILKY_HAIRED_CONJURER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030_7");
		elseif (classId == ENTHUSIASTIC_ARCHER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030_8");
		elseif (classId == EMBITTERED_ARCHER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030_9");
		end
	elseif (sequence == SEQ_025) then
		if (classId == FYE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent045");
			quest:StartSequence(SEQ_030);
		elseif (classId == SILKY_HAIRED_CONJURER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040_2");
		elseif (classId == DISCONCERTED_CONJURER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040_3");
		elseif (classId == SOILEINE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040_4");
		elseif (classId == O_APP_PESI) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040_5");
		elseif (classId == ENIGMATIC_CONJURER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040_6");
		end
	elseif (sequence == SEQ_030) then
		if (classId == O_APP_PESI) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent050");
		elseif (classId == FYE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent045_2");
		elseif (classId == SILKY_HAIRED_CONJURER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040_2");
		elseif (classId == DISCONCERTED_CONJURER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040_3");
		elseif (classId == SOILEINE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040_4");
		elseif (classId == ENIGMATIC_CONJURER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040_6");
		end
	elseif (sequence == SEQ_035) then
		if (classId == MIOUNNE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent050_2");
		end
	elseif (sequence == SEQ_040) then
		if (classId == FYE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent070");
			quest:StartSequence(SEQ_045);
			player:EndEvent();
			quest:UpdateENPCs();
			GetWorldManager():DoZoneChange(player, 153, "PrivateAreaMasterPast", 0, 0, -1947.013, 0.063, -893.221, -1.9);
			return;
		elseif (classId == NIALL) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_2");
		elseif (classId == YDA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_3");
		elseif (classId == PAPALYMO) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_4");
		elseif (classId == FUFUCHA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_5");
		elseif (classId == BURCHARD) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_6");
		elseif (classId == URSBAEN) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_7");
		elseif (classId == NGOLBB) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_8");
		elseif (classId == FARRIMOND) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_9");
		elseif (classId == TKEBBE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_10");
		elseif (classId == HANDELOUP) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_11");
		elseif (classId == GRINNAUX) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_12");
		elseif (classId == SQUEALING_SPRAT) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_13");
		elseif (classId == UNDERPRIVILEGED_URCHIN) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_14");
		elseif (classId == SUGARSTRUNG_SCHOOLGIRL) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_15");
		elseif (classId == DESERTED_DAUGHTER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_16");
		elseif (classId == TROUBLESOME_TOMBOY) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_17");
		elseif (classId == AGING_ELEZEN) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_18");
		elseif (classId == WISPILY_WHISKERED_WOODWORKER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_19");
		elseif (classId == RESPECTABLE_ROEGADYN) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_20");
		elseif (classId == URBANE_ELEZEN) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_21");
		elseif (classId == MERRY_OLD_MATRON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_22");
		elseif (classId == WELL_GROOMED_WOMAN) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_23");
		elseif (classId == GOOD_NATURED_GOODWIFE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_24");
		elseif (classId == OVERANIMATED_HYUR) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_25");
		elseif (classId == FASTIDIOUS_FELLOW) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_26");
		end
	elseif (sequence == SEQ_045) then
		if (classId == YDA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent070_2");
		elseif (classId == PAPALYMO) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent070_3");
		end
	end
	
	player:EndEvent();
	quest:UpdateENPCs();
end

function onPush(player, quest, npc)
	local data = quest:GetData();
	local sequence = quest:getSequence();
	local classId = npc:GetActorClassId();
	
	if (sequence == SEQ_15) then
		if (classId == CNJ_TRIGGER) then
		end
	elseif (sequence == SEQ_035) then
		if (classId == AMPHITHEATRE_TRIGGER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060");
			quest:StartSequence(SEQ_040);
			player:EndEvent();
			quest:UpdateENPCs();
			GetWorldManager():DoZoneChange(player, 206, "PrivateAreaMasterPast", 12, 0, -96.875, 10.586, -1630.849, -3.101);
			return;
		end
	elseif (sequence == SEQ_045) then
		if (classId == FST_ECHO_TRIGGER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent080");
			quest:StartSequence(SEQ_050);
			player:EndEvent();
			quest:UpdateENPCs();
			GetWorldManager():DoZoneChange(player, 155, nil, 0, 0, 60.225, 4.0, -1218.445, 0.862);
			return;
		end
	end
	
	player:EndEvent();
	quest:UpdateENPCs();
end

function onNotice(player, quest, target)
	callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 300, 1, 1, 2);
	player:CompleteQuest(quest);
    callClientFunction(player, "delegateEvent", player, quest, "processEvent080_01", 1);
    player:EndEvent();
    quest:UpdateENPCs();
end

function onNpcLS(player, quest, from, msgStep)
	local sequence = quest:getSequence();
	local msgPack;

	if (from == 1) then
		-- Get the right msg pack
		if (sequence == SEQ_010 or sequence == SEQ_015) then
			msgPack = 1;
		elseif (sequence == SEQ_030 or sequence == SEQ_035) then
			msgPack = 2;
		end	
				
		-- Quick way to handle all msgs nicely.
		player:SendGameMessageLocalizedDisplayName(quest, NPCLS_MSGS[msgPack][msgStep], MESSAGE_TYPE_NPC_LINKSHELL, 1000015);
		if (msgStep >= #NPCLS_MSGS[msgPack]) then
			quest:EndOfNpcLsMsgs();
		else
			quest:ReadNpcLsMsg();
		end
		
		-- Handle anything else
		if (sequence == SEQ_010) then
			quest:StartSequenceForNpcLs(SEQ_015);
		end
	end
	
	player:EndEvent();
end

function getJournalMapMarkerList(player, quest)
	local sequence = quest:getSequence();
	
	if (sequence == SEQ_000) then
		return MRKR_STEP1;
	elseif (sequence == SEQ_003 or sequence == SEQ_004) then
		return MRKR_STEP2;
	elseif (sequence == SEQ_005) then
		return MRKR_STEP3;
	elseif (sequence == SEQ_015) then
		return MRKR_STEP4;
	elseif (sequence == SEQ_020) then
		return MRKR_STEP5;
	elseif (sequence == SEQ_025) then
		return MRKR_STEP6;
	elseif (sequence == SEQ_030) then
		return MRKR_STEP7;
	elseif (sequence == SEQ_035) then
		return MRKR_STEP8;
	elseif (sequence == SEQ_040) then
		return MRKR_STEP9;
	elseif (sequence == SEQ_045) then
		return MRKR_STEP10;
	end	
end

function startDuty(player, quest)
	player:EndEvent();
end