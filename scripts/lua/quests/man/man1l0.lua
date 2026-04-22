require ("global")
require ("quest")

--[[

Quest Script

Name: 	Legends Adrift
Code: 	Man1l0
Id: 	110003
Prereq: Treasures of the Main (Man0l1 - 110002)

]]

-- Sequence Numbers
SEQ_000	= 0;  	-- Echo intance with Y'shtola, Baderon, Etc. Talk to Y'shtola.
SEQ_010	= 10;  	-- Echo instance, talk with Baderon.
SEQ_020	= 20;  	-- Head to MRD guild and talk to Waekbyrt.
SEQ_030	= 30;  	-- Head down the Astalicia to the push trigger.
SEQ_040	= 40;  	-- Head up the Astalicia to the push trigger.
SEQ_050	= 50;	-- Contact Baderon on the Link Pearl.
SEQ_060	= 60;	-- Head to the FSH guild and push the trigger.
SEQ_070	= 70;	-- Head to a spot in Lower La Noscea.
SEQ_080	= 80;	-- Contact Baderon on the Link Pearl.
SEQ_090	= 90;	-- Speak to P'tahjha at the ACN guild.
SEQ_100	= 100;	-- Echo instance, head downstairs to push a trigger and cutscene.
SEQ_110	= 110;	-- Echo instance still, head upstairs to trigger a cutscene.
SEQ_120	= 120;	-- Contact Baderon on the Link Pearl.
SEQ_122	= 122;	-- Head back to Baderon to finish the quest.

-- Quest Actors
BADERON 					= 1000137;
YSHTOLA 					= 1000001;

-- ADV Guild Echo
ADVENTURER					= 1000101;
WHISPERING_ADVENTURER		= 1000102;
UNAPPROACHABLE_ADVENTURER 	= 1000103;
FISH_SMELLING_ADVENTURER	= 1000104;
SPEAR_WIELDING_ADVENTURER	= 1000105;
TRIGGER_ADVGUILD			= 1090080;

-- MRD Guild Echo
WAEKBYRT					= 1000003;
HULKING_CUDA_KNIGHT			= 1000182;
SOPHISTICATED_CUDA_KNIGHT	= 1000108;
FRIGHTENED_CUDA_KNIGHT		= 1000110;
ZEALOUS_PIRATE				= 1000112;
ENRAGED_PIRATE				= 1000113;
TRIGGER_MRD					= 1090081;

-- MRD Guild Echo 2
DISGRUNTLED_PIRATE			= 1000087;
PINE_SCENTED_PIRATE			= 1000088;
BARITONE_PIRATE				= 1000089;
BAYARD						= 1000190;

-- FSH Guild Sequences
NNMULIKA					= 1000153;
SISIPU						= 1000156;
TRIGGER_FSH					= 1090006;
TRIGGER_SEAFLD				= 1090082;

-- ACN Guild Echo
ASSESSOR1			 		= 1000120;
ASSESSOR2			 		= 1000121;
PTAHJHA						= 1000150;
HALDBERK		 			= 1000160;
LILINA			 			= 1000178;
DODOROBA					= 1000196;
IVAN			 			= 1000197;
MERODAULYN		 			= 1000008;
COQUETTISH_PIRATE			= 1000868;
VOLUPTUOUS_PIRATE			= 1000115;
PEACOCKISH_PIRATE			= 1000118;
TRIGGER_ACN_LOWER			= 1090083;
TRIGGER_ACN_UPPER			= 1090084;

-- Quest Markers
MRKR_TRIGGER_FSH			= 11000306;
MRKR_TRIGGER_SEAFLD			= 11000307;
MRKR_TRIGGER_ANC_LOWER		= 11000308;

-- Msg packs for the Npc LS
NPCLS_MSGS = {
	{57, 58, 59}, 	-- SEQ_050
	{92, 93, 94}, 	-- SEQ_070
	{140, 141}		-- SEQ_120
};

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
	GetWorldManager():WarpToPrivateArea(player, "PrivateAreaMasterPast", 3, -430.55, 40.2, 185.41, 1.89);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)
	local data = quest:GetData();
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(BADERON, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
		quest:SetENpc(BADERON);
		quest:SetENpc(ADVENTURER);
		quest:SetENpc(WHISPERING_ADVENTURER);
		quest:SetENpc(UNAPPROACHABLE_ADVENTURER);
		quest:SetENpc(FISH_SMELLING_ADVENTURER);
		quest:SetENpc(SPEAR_WIELDING_ADVENTURER);
		quest:SetENpc(TRIGGER_ADVGUILD, QFLAG_PUSH, false, true);
	elseif (sequence == SEQ_010) then
		quest:SetENpc(BADERON, QFLAG_TALK);
		quest:SetENpc(ADVENTURER);
		quest:SetENpc(WHISPERING_ADVENTURER);
		quest:SetENpc(UNAPPROACHABLE_ADVENTURER);
		quest:SetENpc(FISH_SMELLING_ADVENTURER);
		quest:SetENpc(SPEAR_WIELDING_ADVENTURER);
		quest:SetENpc(YSHTOLA);
	elseif (sequence == SEQ_020) then
		quest:SetENpc(WAEKBYRT, QFLAG_TALK);
		quest:SetENpc(BADERON);
	elseif (sequence == SEQ_030) then
		quest:SetENpc(TRIGGER_MRD, QFLAG_PUSH, false, true);
		quest:SetENpc(HULKING_CUDA_KNIGHT);
		quest:SetENpc(SOPHISTICATED_CUDA_KNIGHT);
		quest:SetENpc(FRIGHTENED_CUDA_KNIGHT);
		quest:SetENpc(ZEALOUS_PIRATE);
		quest:SetENpc(ENRAGED_PIRATE);
		quest:SetENpc(WAEKBYRT);
	elseif (sequence == SEQ_040) then
		quest:SetENpc(TRIGGER_MRD, QFLAG_PUSH, false, true);
		quest:SetENpc(PINE_SCENTED_PIRATE);
		quest:SetENpc(BARITONE_PIRATE);
		quest:SetENpc(BAYARD);
		quest:SetENpc(DISGRUNTLED_PIRATE);
	elseif (sequence == SEQ_060) then
		quest:SetENpc(TRIGGER_FSH, QFLAG_PUSH, false, true);
		quest:SetENpc(BADERON);
	elseif (sequence == SEQ_070) then
		quest:SetENpc(TRIGGER_SEAFLD, QFLAG_PUSH, false, true);
		quest:SetENpc(NNMULIKA);
	elseif (sequence == SEQ_090) then
		quest:SetENpc(PTAHJHA, QFLAG_TALK);
	elseif (sequence == SEQ_100) then
		quest:SetENpc(TRIGGER_ACN_LOWER, QFLAG_PUSH, false, true);
		quest:SetENpc(ASSESSOR1);
		quest:SetENpc(ASSESSOR2);
		quest:SetENpc(HALDBERK);
		quest:SetENpc(LILINA);
		quest:SetENpc(VOLUPTUOUS_PIRATE);
		quest:SetENpc(PEACOCKISH_PIRATE);
		quest:SetENpc(MERODAULYN);
		quest:SetENpc(COQUETTISH_PIRATE);
		quest:SetENpc(IVAN);
	elseif (sequence == SEQ_110) then
		quest:SetENpc(TRIGGER_ACN_UPPER, QFLAG_PUSH, false, true);
	elseif (sequence == SEQ_122) then
		quest:SetENpc(BADERON, QFLAG_REWARD);
	end	
	
end

function onTalk(player, quest, npc)
	local sequence = quest:getSequence();
	local classId = npc:GetActorClassId();

	if (sequence == SEQ_ACCEPT) then
		if (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent200");
			player:EndEvent();
			player:AcceptQuest(quest, true);
			return;
		end
	elseif (sequence == SEQ_000) then		
		seq000_010_onTalk(player, quest, npc, classId);		
	elseif (sequence == SEQ_010) then
		if (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent215");
			player:EndEvent();
			quest:StartSequence(SEQ_020);
			GetWorldManager():WarpToPublicArea(player);
			return;
		elseif (classId == YSHTOLA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent200_8");
		else
			seq000_010_onTalk(player, quest, npc, classId);
		end
	elseif (sequence == SEQ_020) then
		if (classId == WAEKBYRT) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent400");
			quest:StartSequence(SEQ_030);
			player:EndEvent();
			GetWorldManager():WarpToPrivateArea(player, "PrivateAreaMasterPast", 6, -754.03, 7.352, 382.872, 3.133);
			return;
		elseif (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent215_2");
		end
	elseif (sequence == SEQ_030 or sequence == SEQ_040) then
		seq000_030_040_onTalk(player, quest, npc, classId)
	elseif (sequence == SEQ_060) then
		if (classId == NNMULIKA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent600");
		elseif (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent420_2");
		end
	elseif (sequence == SEQ_070) then
		if (classId == NNMULIKA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent600_2");
		end
	elseif (sequence == SEQ_090) then
		if (classId == PTAHJHA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent2000");
			quest:StartSequence(SEQ_100);
			player:EndEvent();
			GetWorldManager():WarpToPrivateArea(player, "PrivateAreaMasterPast", 7);
		elseif (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent610_2");
		end
	elseif (sequence == SEQ_100) then
		seq000_100_onTalk(player, quest, npc, classId)
	elseif (sequence == SEQ_110) then
	elseif (sequence == SEQ_122) then
		if (classId == BADERON) then
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

function seq000_010_onTalk(player, quest, npc, classId)
	if (classId == ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent200_2");
	elseif (classId == WHISPERING_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent200_3");
	elseif (classId == UNAPPROACHABLE_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent200_4");
	elseif (classId == FISH_SMELLING_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent200_5");
	elseif (classId == SPEAR_WIELDING_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent200_6");
	elseif (classId == BADERON) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent200_7");
	end
end

function seq000_030_040_onTalk(player, quest, npc, classId)
	if (classId == HULKING_CUDA_KNIGHT) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent400_2");
	elseif (classId == SOPHISTICATED_CUDA_KNIGHT) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent400_3");
	elseif (classId == FRIGHTENED_CUDA_KNIGHT) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent400_4");
	elseif (classId == ZEALOUS_PIRATE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent400_5");
	elseif (classId == ENRAGED_PIRATE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent400_6");
	elseif (classId == WAEKBYRT) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent400_7");
	elseif (classId == PINE_SCENTED_PIRATE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent410_2");	
	elseif (classId == BARITONE_PIRATE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent410_3");
	elseif (classId == BAYARD) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent410_4");
	elseif (classId == DISGRUNTLED_PIRATE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent410_5");
	end
end

function seq000_100_onTalk(player, quest, npc, classId)
	if (classId == ASSESSOR1) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent2000_2");
	elseif (classId == ASSESSOR2) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent2000_3");
	elseif (classId == HALDBERK) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent2000_4");
	elseif (classId == LILINA) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent2000_5");
	elseif (classId == VOLUPTUOUS_PIRATE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent2000_6");
	elseif (classId == PEACOCKISH_PIRATE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent2000_7");
	elseif (classId == MERODAULYN) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent2000_8");
	elseif (classId == COQUETTISH_PIRATE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent2000_9");
	elseif (classId == 0) then  -- !!!MISSING DIALOG OWNER!!!
		callClientFunction(player, "delegateEvent", player, quest, "processEvent2000_10");
	elseif (classId == 0) then  -- !!!MISSING DIALOG OWNER!!!
		callClientFunction(player, "delegateEvent", player, quest, "processEvent2000_11");
	elseif (classId == IVAN) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent2000_12");
	end
end

function onPush(player, quest, npc)
	local data = quest:GetData();
	local sequence = quest:getSequence();
	local classId = npc:GetActorClassId();
	
	if (sequence == SEQ_000) then
		if (classId == TRIGGER_ADVGUILD) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent210");
			quest:StartSequence(SEQ_010);
		end
	elseif (sequence == SEQ_030) then
		if (classId == TRIGGER_MRD) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent410");
			quest:StartSequence(SEQ_040);
			player:EndEvent();
			GetWorldManager():WarpToPosition(player, -764.519, -3.146, 384.154, 1.575);
			return;
		end
	elseif (sequence == SEQ_040) then
		if (classId == TRIGGER_MRD) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent420");
			quest:NewNpcLsMsg(1);
			quest:StartSequence(SEQ_050);
			player:EndEvent();
			GetWorldManager():WarpToPublicArea(player);
			return;
		end
	elseif (sequence == SEQ_060) then
		if (classId == TRIGGER_FSH) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent600");
			quest:StartSequence(SEQ_070);
		end
	elseif (sequence == SEQ_070) then
		if (classId == TRIGGER_SEAFLD) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent610");
			quest:NewNpcLsMsg(1);
			quest:StartSequence(SEQ_080);
		end
	elseif (sequence == SEQ_100) then
		if (classId == TRIGGER_ACN_LOWER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent2001");
			quest:StartSequence(SEQ_110);
			player:EndEvent();
			GetWorldManager():WarpToPosition(player, -785.938, -0.62, 189.044, 3.09);
			return;
		end
	elseif (sequence == SEQ_110) then
		if (classId == TRIGGER_ACN_UPPER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent2002");
			quest:NewNpcLsMsg(1);
			quest:StartSequence(SEQ_120);
			player:EndEvent();
			GetWorldManager():WarpToPublicArea(player);
			return;
		end
	end	
	
	player:EndEvent();
	quest:UpdateENPCs();
end

function onNpcLS(player, quest, from, msgStep)
	local sequence = quest:getSequence();
	local msgPack;

	if (from == 1) then
		-- Get the right msg pack
		if (sequence == SEQ_050 or sequence == SEQ_060) then
			msgPack = 1;
		elseif (sequence == SEQ_080 or sequence == SEQ_090) then
			msgPack = 2;
		elseif (sequence == SEQ_120 or sequence == SEQ_122) then
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
		if (sequence == SEQ_050) then
			quest:StartSequenceForNpcLs(SEQ_060);
		elseif (sequence == SEQ_080) then
			quest:StartSequenceForNpcLs(SEQ_090);
		elseif (sequence == SEQ_120) then
			quest:StartSequenceForNpcLs(SEQ_122);
		end
	end
	
	player:EndEvent();
end

function getJournalMapMarkerList(player, quest)
	local sequence = quest:getSequence();
	
end