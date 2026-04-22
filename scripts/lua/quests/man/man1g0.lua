require ("global")
require ("quest")

--[[

Quest Script

Name: 	Whispers in the Wood
Code: 	Man1g0
Id: 	110007
Prereq: Souls Gone Wild (Man0g1 - 110006), Level 8

]]

-- Sequence Numbers
SEQ_000	= 0;  	-- Visit the CRP guild.
SEQ_005	= 5;  	-- Go to Acorn Orchard in echo.
SEQ_010	= 10;  	-- Speak to Fye in echo.
SEQ_015	= 15;  	-- Return to CRP guild in echo.
SEQ_020	= 20;  	-- Talk to Miounne on the LS.
SEQ_025	= 25;	-- Go to BTN guild.
SEQ_030	= 30;	-- Go to the Moogle in the South Shroud.
SEQ_035	= 35;	-- Move to the Moogle in echo.
SEQ_040	= 40;	-- Return to the BTN guild.
SEQ_045	= 45;	-- Talk to Miounne on the LS.
SEQ_050	= 50;	-- Visit the ARC guild.
SEQ_055	= 55;	-- Go deeper into the ARC guild.
SEQ_060	= 60;	-- Talk to Miounne on the LS.
SEQ_065	= 65;	-- Return to Miounne.

-- Actor Class Ids
MIOUNNE 					= 1000230;

-- CRP Guild Echo
ZEZEKUTA					= 1000240;
ANAIDJAA					= 1000465;
CAPLAN						= 1000822;
FRANCES						= 1000466;
ULMHYLT						= 1000823;
DECIMA						= 1000622;
CHALYO_TAMLYO				= 1000623;
PLAYGROUND_TRIGGER			= 1090204;

-- Acorn Orchard Echo
FYE							= 1000014;
TROUBLESOME_TOMBOY			= 1000828;
DESERTED_DAUGHTER			= 1000827;
PLAYGROUND_EXIT_TRIGGER		= 1090205;

-- West Gridania Echo + BTN
BTN_TRIGGER					= 1090046;
WEST_SHROUD_TRIGGER			= 1090067;
PUDGY_MOOGLE				= 1000328;
SHROUD_ECHO_TRIGGER			= 0;
OPYLTYL						= 1000236;

-- ARC Guild
NONOLATO					= 1000463;
GUILD_ARC_INSIDE_TRIGGER	= 1090068;

-- Msg packs for the Npc LS
NPCLS_MSGS = {
	{96, 97, 98},		-- SEQ_020
	{170, 171, 172},	-- SEQ_045
	{231, 232}			-- SEQ_060
};

-- Quest Markers
MRKR_STEP1				= 11000701;
MRKR_STEP2				= 11000702;
MRKR_STEP3				= 11000703;
MRKR_STEP4				= 11000704;
MRKR_STEP5				= 11000705;
MRKR_STEP6				= 11000706;
MRKR_STEP7				= 11000707;
MRKR_STEP8				= 11000708;
MRKR_STEP9				= 11000709;
MRKR_STEP10				= 11000710;
MRKR_STEP11				= 11000711;
MRKR_STEP12				= 11000712;

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
		quest:SetENpc(ANAIDJAA, QFLAG_TALK);
		quest:SetENpc(MIOUNNE);
	elseif (sequence == SEQ_005) then
		quest:SetENpc(ANAIDJAA);
		quest:SetENpc(CAPLAN);
		quest:SetENpc(FRANCES);
		quest:SetENpc(ULMHYLT);
		quest:SetENpc(DECIMA);
		quest:SetENpc(CHALYO_TAMLYO);
		quest:SetENpc(PLAYGROUND_TRIGGER, QFLAG_PUSH, false, true);
	elseif (sequence == SEQ_010) then
		quest:SetENpc(FYE, QFLAG_TALK);
		quest:SetENpc(TROUBLESOME_TOMBOY);
		quest:SetENpc(DESERTED_DAUGHTER);
	elseif (sequence == SEQ_015) then
		quest:SetENpc(TROUBLESOME_TOMBOY);
		quest:SetENpc(DESERTED_DAUGHTER);
		quest:SetENpc(PLAYGROUND_EXIT_TRIGGER, QFLAG_PUSH, false, true);
	elseif (sequence == SEQ_020) then
	elseif (sequence == SEQ_025) then
		quest:SetENpc(BTN_TRIGGER, QFLAG_PUSH, false, true);
		quest:SetENpc(MIOUNNE);
	elseif (sequence == SEQ_030) then
		quest:SetENpc(MIOUNNE);
	elseif (sequence == SEQ_035) then
	elseif (sequence == SEQ_040) then
		quest:SetENpc(OPYLTYL, QFLAG_TALK);
	elseif (sequence == SEQ_045) then
	elseif (sequence == SEQ_050) then
		quest:SetENpc(NONOLATO, QFLAG_TALK);
		quest:SetENpc(MIOUNNE);
	elseif (sequence == SEQ_055) then
		quest:SetENpc(GUILD_ARC_INSIDE_TRIGGER, QFLAG_PUSH, false, true);
		quest:SetENpc(NONOLATO);
	elseif (sequence == SEQ_065) then
		quest:SetENpc(MIOUNNE, QFLAG_REWARD);
		quest:SetENpc(NONOLATO);
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
		if (classId == ANAIDJAA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
			player:EndEvent();
			quest:StartSequence(SEQ_005);
			GetWorldManager():WarpToPrivateArea(player, "PrivateAreaMasterPast", 7);
			return;
		elseif (classId == MIOUNNE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent000_2");
		end
	elseif (sequence == SEQ_005) then
		if (classId == ZEZEKUTA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010_2");
		elseif (classId == ANAIDJAA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010_3")
		elseif (classId == FRANCES) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010_4");
		elseif (classId == CAPLAN) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010_5");
		elseif (classId == ULMHYLT) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010_6");
		elseif (classId == DECIMA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010_7");
		elseif (classId == CHALYO_TAMLYO) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010_8");
		end
	elseif (sequence == SEQ_010) then
		if (classId == ZEZEKUTA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020_2");
		elseif (classId == DESERTED_DAUGHTER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020_3")
		elseif (classId == TROUBLESOME_TOMBOY) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020_4");
		elseif (classId == FYE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030");
			player:EndEvent();
			quest:StartSequence(SEQ_015);
			GetWorldManager():DoZoneChange(player, 206, "PrivateAreaMasterPast", 9, 15, -35.566, 7.845, -1250.233, 0.842);	
		end
	elseif (sequence == SEQ_015) then
		if (classId == DESERTED_DAUGHTER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020_3");
		elseif (classId == TROUBLESOME_TOMBOY) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020_4")
		end
	elseif (sequence == SEQ_020) then
	elseif (sequence == SEQ_025) then
		if (classId == MIOUNNE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040_2");
		end
	elseif (sequence == SEQ_030) then
		if (classId == MIOUNNE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent050_2");
		end
	elseif (sequence == SEQ_035) then
		if (classId == MIOUNNE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent050_2");
		end
	elseif (sequence == SEQ_040) then
		if (classId == OPYLTYL) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent080");
			quest:NewNpcLsMsg(1);
			quest:StartSequence(SEQ_045);
		end
	elseif (sequence == SEQ_050) then
		if (classId == NONOLATO) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent090");
			quest:StartSequence(SEQ_055);
		elseif (classId == MIOUNNE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent080_3");
		end
	elseif (sequence == SEQ_055) then
		if (classId == NONOLATO) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent090_2");
		end
	elseif (sequence == SEQ_065) then
		if (classId == MIOUNNE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventComplete");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 300, 1, 1, 2);
			player:EndEvent();
			player:CompleteQuest(quest);
			return;
		end
	end	
	
	player:EndEvent();
	quest:UpdateENPCs();
end

function onPush(player, quest, npc)
	local data = quest:GetData();
	local sequence = quest:getSequence();
	local classId = npc:GetActorClassId();
	
	if (sequence == SEQ_005) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent020");        
        player:EndEvent();
		quest:StartSequence(SEQ_010);
		GetWorldManager():DoZoneChange(player, 206, "PrivateAreaMasterPast", 8, 15, -35.452, 7.845, -1250.145, 0.777);	
	elseif (sequence == SEQ_015) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent040");        
        player:EndEvent();
		quest:NewNpcLsMsg(1);
		quest:StartSequence(SEQ_020);
		GetWorldManager():DoZoneChange(player, 206, nil, 0, 15, 15.593, 8.75, -1266.0, -1.322);	
	elseif (sequence == SEQ_020) then
	elseif (sequence == SEQ_025) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050");    
		quest:StartSequence(SEQ_030);
	elseif (sequence == SEQ_030) then
		-- Go to west shroud
		callClientFunction(player, "delegateEvent", player, quest, "processEvent060");    
		quest:StartSequence(SEQ_035);
	elseif (sequence == SEQ_035) then
		-- Go to moogle
		callClientFunction(player, "delegateEvent", player, quest, "processEvent070");    
		quest:StartSequence(SEQ_040);
	elseif (sequence == SEQ_055) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent100"); 
		quest:NewNpcLsMsg(1);   
		quest:StartSequence(SEQ_060);
	end	
	
	player:EndEvent();
	quest:UpdateENPCs();
end

function onNpcLS(player, quest, from, msgStep)
	local sequence = quest:getSequence();
	local msgPack;

	if (from == 1) then
		-- Get the right msg pack
		if (sequence == SEQ_020 or sequence == SEQ_025) then
			msgPack = 1;
		elseif (sequence == SEQ_045 or sequence == SEQ_050) then
			msgPack = 2;
		elseif (sequence == SEQ_060 or sequence == SEQ_065) then
			msgPack = 3;
		end	
				
		-- Quick way to handle all msgs nicely.
		player:SendGameMessageLocalizedDisplayName(quest, NPCLS_MSGS[msgPack][msgStep], MESSAGE_TYPE_NPC_LINKSHELL, 1000015);
		if (msgStep >= #NPCLS_MSGS[msgPack]) then
			quest:EndOfNpcLsMsgs();
		else
			quest:ReadNpcLsMsg();
		end
		
		-- Handle anything else
		if (sequence == SEQ_020) then
			quest:StartSequenceForNpcLs(SEQ_025);
		elseif (sequence == SEQ_045) then
			quest:StartSequenceForNpcLs(SEQ_050);
		elseif (sequence == SEQ_060) then
			quest:StartSequenceForNpcLs(SEQ_065);
		end
	end
	
	player:EndEvent();
end

function getJournalMapMarkerList(player, quest)
	local sequence = quest:getSequence();
	
	if (sequence == SEQ_000) then
		return MRKR_STEP1;
	elseif (sequence == SEQ_005) then
		return MRKR_STEP2;
	elseif (sequence == SEQ_010) then
		return MRKR_STEP3;
	elseif (sequence == SEQ_015) then
		return MRKR_STEP4;
	elseif (sequence == SEQ_025) then
		return MRKR_STEP6;
	elseif (sequence == SEQ_030) then
		return MRKR_STEP7;
	elseif (sequence == SEQ_035) then
		return MRKR_STEP8;
	elseif (sequence == SEQ_040) then
		return MRKR_STEP9;
	elseif (sequence == SEQ_050) then
		return MRKR_STEP11;
	elseif (sequence == SEQ_055) then
		return MRKR_STEP12;
	elseif (sequence == SEQ_065) then
		return MRKR_STEP13
	end	
end