require ("global")
require ("quest")

--[[

Quest Script

Name: 	Together We Stand
Code: 	Man206
Id: 	110014
Prereq: Man200 complete. Level 22.

]]

-- Sequence Numbers
SEQ_000	= 0;  	-- Enter the Waking Sands and talk to Minfilia.
SEQ_005	= 5;  	-- Contact your Path Companion.
SEQ_010	= 10;  	-- Meet your Path Companion in Gridania at the Aetheryte.
SEQ_015	= 15;  	-- Go to Camp Nine Ivies and start the duty.
SEQ_020	= 20;  	-- DUTY: Head to Moonspore Grove, grabbing a podling.
SEQ_025	= 25;	-- DUTY: Head back with the pod and clear the duty.
SEQ_030	= 30;	-- Pray, return to the Waking Sands.

-- Quest Actors
MINFILIA				= 1000843;
TATARU					= 1001046;
SAHJA_ZHWAN				= 1001373;
NENEKANI				= 1001374;
GODFREY					= 1001375;
FENANA					= 1001376;
NONORU					= 1001377;
SERANELIEN				= 1001378;

TROUBLED_TRADER			= 1000812;
SOIL_SCENTED_BOTANIST	= 1000835;
LONG_LEGGED_LADY		= 1001112;
NONCHALANT_GOLDSMITH	= 1001015;
STERN_FACED_SEAWOLF		= 1001222;
SATZFLOH				= 1001228;
PERCEVAINS				= 1001229;
UNA_TAYUUN				= 1001230;
BURLY_VOICED_BRUTE		= 1001379;

ALMXIO					= 1001085;
ZOXIO					= 1001086;
DILUXIO					= 1001178;
DOKIXIA					= 1001238;

MARKET_ENTRENCE			= 1090265;
EVENT_DOOR_EXIT			= 1090160;
EVENT_DOOR_OFFICE_W		= 1090161;
EVENT_DOOR_OFFICE_E		= 1090162;

GRIDANIA_TRIGGER		= 1090255;
DUTY_START_TRIGGER		= 1;
DUTY_CUTSCENE_TRIGGER	= 1;

-- Msg packs for the Npc LS. Only used for SEQ_005 but different ones for different SNPCs.
NPCLS_MSGS = {
	{330, 339, 348, 357}, 	-- Nine versions; add skin index for proper msgs for the snpc race.
};

-- Quest Markers
MRKR_GRIDANIA			= 11001402;
MRKR_NINE_IVES			= 11001403;
MRKR_DUTY_MID			= 11001404;
MRKR_DOKIXIA			= 11001405;
MRKR_FLAXIO				= 11001406;
MRKR_TATARU				= 11001407;

-- Quest Flags
FLAG_ENTERED_OFFICE		= 0;

function onStart(player, quest)
	quest:NewNpcLsMsg(6);
	quest:StartSequence(SEQ_005);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)
	local data = quest:GetData();
	
	-- ENPCs that appear before accepting it.
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(MINFILIA, QFLAG_TALK);
		quest:SetENpc(TATARU);
		quest:SetENpc(SERANELIEN);
		quest:SetENpc(SAHJA_ZHWAN);
		quest:SetENpc(NENEKANI);
		quest:SetENpc(GODFREY);
		quest:SetENpc(FENANA);
		quest:SetENpc(NONORU);
		quest:SetENpc(ALMXIO);
		quest:SetENpc(ZOXIO);
		quest:SetENpc(DILUXIO);
		quest:SetENpc(EVENT_DOOR_OFFICE_W, QFLAG_PUSH, false, true);
		quest:SetENpc(EVENT_DOOR_OFFICE_E, QFLAG_NONE, false, true);
	end
	
	-- These ENPCs appear between accepting the quest and before completing it.
	if (sequence >= SEQ_005 and sequence < SEQ_030) then	
		quest:SetENpc(TATARU);
		quest:SetENpc(TROUBLED_TRADER);
		quest:SetENpc(SOIL_SCENTED_BOTANIST);
		quest:SetENpc(LONG_LEGGED_LADY);
		quest:SetENpc(NONCHALANT_GOLDSMITH);
		quest:SetENpc(STERN_FACED_SEAWOLF);
		quest:SetENpc(SATZFLOH);
		quest:SetENpc(PERCEVAINS);
		quest:SetENpc(UNA_TAYUUN);
		quest:SetENpc(BURLY_VOICED_BRUTE);
	end
	
	-- These ENPCs appear through the whole quest
	if (sequence >= SEQ_005) then
		quest:SetENpc(SERANELIEN);
		quest:SetENpc(SAHJA_ZHWAN);
		quest:SetENpc(NENEKANI);
		quest:SetENpc(GODFREY);
		quest:SetENpc(FENANA);
		quest:SetENpc(NONORU);
	end
	
	-- Sequence Specific ENPCs
	if (sequence == SEQ_010) then
		quest:SetENpc(GRIDANIA_TRIGGER, QFLAG_PUSH, false, true);
	elseif (sequence == SEQ_015) then
		quest:SetENpc(DUTY_START_TRIGGER, QFLAG_PUSH, false, true);	
	elseif (sequence == SEQ_030) then
		quest:SetENpc(TATARU, QFLAG_REWARD);
	end
	
	-- Duty ENPCs
	if (sequence == SEQ_020 or sequence == SEQ_025) then
		quest:SetENpc(ALMXIO);
		quest:SetENpc(ZOXIO);
		quest:SetENpc(DILUXIO);
		quest:SetENpc(DOKIXIA);
		quest:SetENpc(DUTY_CUTSCENE_TRIGGER, QFLAG_NONE, false, true);
	end
		
	-- Waking Sands
	quest:SetENpc(MARKET_ENTRENCE, sequence == SEQ_ACCEPT and QFLAG_PUSH or QFLAG_NONE, false, true);
	quest:SetENpc(EVENT_DOOR_EXIT, QFLAG_NONE, false, true);
end

function onTalk(player, quest, npc)
	local sequence = quest:getSequence();
	local classId = npc:GetActorClassId();

	if (sequence == SEQ_ACCEPT) then
		if (classId == MINFILIA) then
			local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEvent001");
			if (questAccepted == 1) then
				player:EndEvent();
				player:AcceptQuest(quest, true);
				GetWorldManager():DoZoneChange(player, 181, "PrivateAreaMasterPast", 2, 15, -142.75, 1, -160, -1.6);				
				return;
			end
		end
	elseif (sequence == SEQ_005) then
		if (classId == TATARU) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent001_2");			
		end
	elseif (sequence == SEQ_015) then
	elseif (sequence == SEQ_020 or sequence == SEQ_025) then
		if (classId == ZOXIO) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent016_1");
		elseif (classId == ALMXIO) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent016_2");
		elseif (classId == DILUXIO) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent016_3");
		elseif (classId == DOKIXIA) then
			if (sequence == SEQ_020) then
				callClientFunction(player, "delegateEvent", player, quest, "pE20", player:GetSNpcNickname(), player:GetSNpcSkin(), player:GetSNpcPersonality(), player:GetSNpcCoordinate(), player:GetInitialTown());
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent020_2");
			end
		end
	elseif (sequence == SEQ_030) then
		if (classId == TATARU) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 300, 1, 1, 2);
			player:EndEvent();
			player:CompleteQuest(quest);
			return;
		end
	end
	
	-- For all the other ENPCs
	if (sequence == SEQ_ACCEPT) then
		seqAcc_onTalkOtherNpcs(player, quest, classId);
	elseif (sequence >= SEQ_005 and sequence <= SEQ_025) then
		if (not (sequence == SEQ_005) and classId == TATARU) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010_2", player:GetSNpcNickname());
		end
		seq005_onTalkOtherNpcs(player, quest, classId);
	elseif (sequence == SEQ_030) then
		seq030_onTalkOtherNpcs(player, quest, classId);
	end
	
	player:EndEvent();
	quest:UpdateENPCs();
end

function seqAcc_onTalkOtherNpcs(player, quest, classId)
	if (classId == TATARU) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_2");
	elseif (classId == SERANELIEN) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_3");
	elseif (classId == SAHJA_ZHWAN) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_4");
	elseif (classId == NENEKANI) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_5");
	elseif (classId == GODFREY) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_6");
	elseif (classId == FENANA) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_7");
	elseif (classId == NONORU) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_8");
	elseif (classId == ALMXIO) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_9");
	elseif (classId == ZOXIO) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_10");
	elseif (classId == DILUXIO) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_11");
	end
end

function seq005_onTalkOtherNpcs(player, quest, classId)
	if (classId == SERANELIEN) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent001_3");
	elseif (classId == SAHJA_ZHWAN) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent001_4");
	elseif (classId == GODFREY) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent001_5");
	elseif (classId == FENANA) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent001_6");
	elseif (classId == NONORU) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent001_7");
	elseif (classId == SOIL_SCENTED_BOTANIST) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent001_8");
	elseif (classId == LONG_LEGGED_LADY) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent001_9");			
	elseif (classId == TROUBLED_TRADER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent001_10");
	elseif (classId == NONCHALANT_GOLDSMITH) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent012_2");
	elseif (classId == STERN_FACED_SEAWOLF) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent012_3");
	elseif (classId == BURLY_VOICED_BRUTE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent012_4");
	elseif (classId == SATZFLOH) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent012_5");
	elseif (classId == PERCEVAINS) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent012_6");
	elseif (classId == UNA_TAYUUN) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent012_7");
	end
end

function seq030_onTalkOtherNpcs(player, quest, classId)	
	if (classId == SERANELIEN) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent030_2");
	elseif (classId == SAHJA_ZHWAN) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent030_3");
	elseif (classId == NENEKANI) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_5");
	elseif (classId == GODFREY) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent030_4");
	elseif (classId == FENANA) then	
		callClientFunction(player, "delegateEvent", player, quest, "processEvent030_5");
	elseif (classId == NONORU) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent030_6");
	end
end

function onPush(player, quest, npc)
	local data = quest:GetData();
	local sequence = quest:getSequence();
	local classId = npc:GetActorClassId();
	local currentPrivateArea = 1;	
	
	-- Entrance/Exit to the Waking Sands
	if (classId == MARKET_ENTRENCE) then		
		if (sequence == SEQ_ACCEPT) then
			currentPrivateArea = 1; -- Quest Beginning
		elseif (sequence == SEQ_005) then
			currentPrivateArea = 2; -- Post-Accept
		elseif (sequence == SEQ_030) then
			currentPrivateArea = 3; -- Quest Completion
		end
	
		GetWorldManager():DoZoneChange(player, 181, "PrivateAreaMasterPast", currentPrivateArea, 15, -205.25, 0, -160, 1.55);		
		player:EndEvent();
		return;
	elseif (classId == EVENT_DOOR_EXIT) then
		player:EndEvent();
		GetWorldManager():DoZoneChange(player, 175, nil, 0, 15, -216.52, 190, 30.5, 2.32);
	end
	
	-- Mid-Sequence Push Events
	if (sequence == SEQ_ACCEPT or sequence == SEQ_005) then
		if (classId == EVENT_DOOR_OFFICE_W) then
			if (sequence == SEQ_ACCEPT) then
				if (not quest:GetData():GetFlag(FLAG_ENTERED_OFFICE)) then
					callClientFunction(player, "delegateEvent", player, quest, "processEventUdowntownrectStart");
					quest:GetData():SetFlag(FLAG_ENTERED_OFFICE);
				end
			end
			player:EndEvent();
			GetWorldManager():WarpToPosition(player, -126.2, 1.2, -160, 1.6);
			return;
		elseif (classId == EVENT_DOOR_OFFICE_E) then
			player:EndEvent();
			GetWorldManager():WarpToPosition(player, -142.75, 1, -160, -1.6);
			return;
		end
	elseif (sequence == SEQ_010) then
		if (classId == GRIDANIA_TRIGGER) then
			callClientFunction(player, "delegateEvent", player, quest, "pE12", player:GetSNpcNickname(), player:GetSNpcSkin(), player:GetSNpcPersonality(), player:GetSNpcCoordinate(), player:GetInitialTown());
			quest:StartSequence(SEQ_015);
		end
	end
	
	player:EndEvent();
	quest:UpdateENPCs();
end

function onNpcLS(player, quest, from, msgStep)
	local sequence = quest:getSequence();
	local msgPack;

	if (from == 6) then
		-- Get the right msg pack
		if (sequence == SEQ_005 or sequence == SEQ_010) then
			msgPack = 1;
		end	
				
		-- Quick way to handle all msgs nicely.
		player:SendGameMessageLocalizedDisplayName(quest, NPCLS_MSGS[msgPack][msgStep] + player:GetSNpcSkin(), MESSAGE_TYPE_NPC_LINKSHELL, 1000015);
		if (msgStep >= #NPCLS_MSGS[msgPack]) then
			quest:EndOfNpcLsMsgs();
		else
			quest:ReadNpcLsMsg();
		end
		
		-- Handle anything else
		if (sequence == SEQ_005) then
			quest:StartSequenceForNpcLs(SEQ_010);
		end
	end
	
	player:EndEvent();
end

function getJournalMapMarkerList(player, quest)
	local sequence = quest:getSequence();
	local data = quest:GetData();
	
	if (sequence == SEQ_010) then
		return MRKR_GRIDANIA;
	elseif (sequence == SEQ_015) then
		return MRKR_NINE_IVES;
	elseif (sequence == SEQ_020) then
	elseif (sequence == SEQ_025) then
		return MRKR_FLAXIO;
	elseif (sequence == SEQ_030) then
		return MRKR_TATARU;
	end
end

function getJournalInformation(player, quest)
	return 0, 0, 0, 0, 0, player:GetSNpcNickname();
end
