require ("global")
require ("quest")

--[[

Quest Script

Name: 	Shapeless Melody
Code: 	Man0l0
Id: 	110001
Prereq: None (Given on chara creation)

]]

-- Sequence Numbers
SEQ_000	= 0;  -- On the boat interior; contains the basics tutorial.
SEQ_005	= 5;  -- Combat on the top of the boat.
SEQ_010	= 10; -- In Limsa Lominsa's port.

-- Actor Class Ids
WELLTRAVELED_MERCHANT 	= 1000438;
TIPSY_ADVENTURER 		= 1000439;
CULTIVATED_TENDER 		= 1000440;
ANXIOUS_ADVENTURER 		= 1000441;
BABYFACED_ADVENTURER 	= 1000442;
AUSTERE_ADVENTURER 		= 1000443;
UNDIGNIFIED_ADVENTURER 	= 1000444;
SHADOWY_TRAVELER 		= 1000445;
ASTUTE_MERCHANT 		= 1000446;
VOLUPTUOUS_VIXEN 		= 1000447;
INDIFFERENT_PASSERBY 	= 1000448;
PRATTLING_ADVENTURER 	= 1000449;
LANKY_TRAVELER 			= 1000450;
GRINNING_ADVENTURER 	= 1000451;
ROSTNSTHAL 				= 1001652;
EXIT_TRIGGER 			= 1090025;

HOB						= 1000151;
GERT					= 1500004;
LORHZANT				= 1500005;
MUSCLEBOUND_DECKHAND	= 1000261;
PEARLYTOOTHED_PORTER	= 1000260;
--PASTYFACED_ADVENTURER	= 1000264; -- Missing?
PRIVAREA_PAST_EXIT		= 1290002;

-- Quest Markers
MRKR_HOB					= 11000202;
MRKR_ROSTNSTHAL 			= 11000203;
MRKR_VOLUPTUOUS_VIXEN 		= 11000204;
MRKR_BABYFACED_ADVENTURER 	= 11000205;
MRKR_TRIGGER_DOOR			= 11000206;

-- Quest Flags
FLAG_SEQ000_MINITUT0	= 0;
FLAG_SEQ000_MINITUT1	= 1;
FLAG_SEQ000_MINITUT2	= 2;
FLAG_SEQ000_MINITUT3	= 3;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)
	if (sequence == SEQ_000) then		
		-- Setup states incase we loaded in.
		local data = quest:GetData();
		
		local rostnsthalFlag = data:GetFlag(FLAG_SEQ000_MINITUT1) and QFLAG_NONE or QFLAG_TALK;
		local vixenFlag = data:GetFlag(FLAG_SEQ000_MINITUT2) and QFLAG_NONE or QFLAG_TALK;
		local babyfaceFlag = data:GetFlag(FLAG_SEQ000_MINITUT3) and QFLAG_NONE or QFLAG_TALK;
		local rostnsthalCanPush = not data:GetFlag(FLAG_SEQ000_MINITUT0);
		local exitCanPush = data:GetFlags() == 0xF;
		local exitFlag = data:GetFlags() == 0xF and QFLAG_PUSH or QFLAG_NONE;		
		
		quest:SetENpc(WELLTRAVELED_MERCHANT);
		quest:SetENpc(TIPSY_ADVENTURER);
		quest:SetENpc(CULTIVATED_TENDER);
		quest:SetENpc(ANXIOUS_ADVENTURER);
		quest:SetENpc(BABYFACED_ADVENTURER, babyfaceFlag);
		quest:SetENpc(AUSTERE_ADVENTURER);
		quest:SetENpc(UNDIGNIFIED_ADVENTURER);
		quest:SetENpc(SHADOWY_TRAVELER);
		quest:SetENpc(ASTUTE_MERCHANT);
		quest:SetENpc(VOLUPTUOUS_VIXEN, vixenFlag);
		quest:SetENpc(INDIFFERENT_PASSERBY);
		quest:SetENpc(PRATTLING_ADVENTURER);
		quest:SetENpc(LANKY_TRAVELER);
		quest:SetENpc(GRINNING_ADVENTURER);
		quest:SetENpc(ROSTNSTHAL, rostnsthalFlag, true, rostnsthalCanPush);
		quest:SetENpc(EXIT_TRIGGER, exitFlag, false, exitCanPush);
		print(tostring(exitCanPush));
	elseif (sequence == SEQ_005) then
	elseif (sequence == SEQ_010) then		
		quest:SetENpc(HOB, QFLAG_TALK);
		quest:SetENpc(GERT);
		quest:SetENpc(LORHZANT);
		quest:SetENpc(MUSCLEBOUND_DECKHAND);
		quest:SetENpc(PEARLYTOOTHED_PORTER);
		quest:SetENpc(UNDIGNIFIED_ADVENTURER);
		quest:SetENpc(WELLTRAVELED_MERCHANT);
		quest:SetENpc(VOLUPTUOUS_VIXEN);
		quest:SetENpc(LANKY_TRAVELER);
		quest:SetENpc(PRIVAREA_PAST_EXIT, QFLAG_NONE, false, true);
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
	
	if (sequence == SEQ_000) then
		if (classId == EXIT_TRIGGER) then
			doExitDoor(player, quest, npc);
			return;
		elseif (classId == ROSTNSTHAL) then
			callClientFunction(player, "delegateEvent", player, quest, "processTtrNomal002");
			player:EndEvent();
		end
	elseif (sequence == SEQ_010) then
		if (classId == PRIVAREA_PAST_EXIT) then
			if (eventName == "caution") then
				worldMaster = GetWorldMaster();
				player:SendGameMessage(player, worldMaster, 34109, 0x20);
			elseif (eventName == "exit") then		
			end
		end
	end
	
	quest:UpdateENPCs();
end

function onNotice(player, quest, target)
	local sequence = quest:getSequence();
	
	if (sequence == SEQ_000) then		
		callClientFunction(player, "delegateEvent", player, quest, "processTtrNomal001withHQ");		
	end
	
	player:EndEvent();	
	quest:UpdateENPCs();
end

function seq000_onTalk(player, quest, npc, classId)	
	local data = quest:GetData();

	if (classId == WELLTRAVELED_MERCHANT) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_4");
	elseif (classId == TIPSY_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_5");
	elseif (classId == CULTIVATED_TENDER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_6");
	elseif (classId == ANXIOUS_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_7");
	elseif (classId == BABYFACED_ADVENTURER) then
		if (not data:GetFlag(FLAG_SEQ000_MINITUT3)) then
			callClientFunction(player, "delegateEvent", player, quest, "processTtrMini003");
			data:SetFlag(FLAG_SEQ000_MINITUT3);
		else
			callClientFunction(player, "delegateEvent", player, quest, "processEvent000_8");
		end
	elseif (classId == AUSTERE_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_9");
	elseif (classId == UNDIGNIFIED_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_10");
	elseif (classId == SHADOWY_TRAVELER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_11");
	elseif (classId == ASTUTE_MERCHANT) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_12");
	elseif (classId == VOLUPTUOUS_VIXEN) then
		if (not data:GetFlag(FLAG_SEQ000_MINITUT2)) then
			callClientFunction(player, "delegateEvent", player, quest, "processTtrMini002");
			data:SetFlag(FLAG_SEQ000_MINITUT2);
		else
			callClientFunction(player, "delegateEvent", player, quest, "processEvent000_13");
		end
	elseif (classId == INDIFFERENT_PASSERBY) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_14");
	elseif (classId == PRATTLING_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_15");
	elseif (classId == LANKY_TRAVELER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_16");
	elseif (classId == GRINNING_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_17");
	elseif (classId == ROSTNSTHAL) then
		-- Handle the talk tutorial after the push one.
		if (not data:GetFlag(FLAG_SEQ000_MINITUT0)) then
			callClientFunction(player, "delegateEvent", player, quest, "processTtrNomal003");
			data:SetFlag(FLAG_SEQ000_MINITUT0);		
		else
			callClientFunction(player, "delegateEvent", player, quest, "processTtrMini001");
			if (not data:GetFlag(FLAG_SEQ000_MINITUT1)) then
				data:SetFlag(FLAG_SEQ000_MINITUT1);
			end
		end
	end	
		
	player:EndEvent();
end

function seq010_onTalk(player, quest, npc, classId)	
	local data = quest:GetData();

	if (classId == MUSCLEBOUND_DECKHAND) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent020_2");
	elseif (classId == PEARLYTOOTHED_PORTER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent020_3");
	elseif (classId == UNDIGNIFIED_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent020_5");
	elseif (classId == VOLUPTUOUS_VIXEN) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent020_6");
	elseif (classId == WELLTRAVELED_MERCHANT) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent020_7");
	elseif (classId == LANKY_TRAVELER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent020_8");
	elseif (classId == HOB) then
		local choice = callClientFunction(player, "delegateEvent", player, quest, "processEvent020_9");
		if (choice == 1) then
			player:ReplaceQuest(quest, "Man0l1");
			return;
		end
	elseif (classId == GERT) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent020_10");
	elseif (classId == LORHZANT) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent020_11");
	end
	
	player:EndEvent();
end

function getJournalMapMarkerList(player, quest)
	local sequence = quest:getSequence();
	
	if (sequence == SEQ_000) then
		return MRKR_ROSTNSTHAL, MRKR_BABYFACED_ADVENTURER, MRKR_VOLUPTUOUS_VIXEN;
	elseif (sequence == SEQ_010) then
		return MRKR_HOB;
	end
end

function doExitDoor(player, quest, npc)
	local choice = callClientFunction(player, "delegateEvent", player, quest, "processEventNewRectAsk", nil);	
	if (choice == 1) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent000_2", nil);
		player:EndEvent();
		
		quest:StartSequence(SEQ_005);
		
		contentArea = player.CurrentArea:CreateContentArea(player, "/Area/PrivateArea/Content/PrivateAreaMasterSimpleContent", "man0l01", "SimpleContent30002", "Quest/QuestDirectorMan0l001");
		
		if (contentArea == nil) then
			return;
		end
		
		director = contentArea:GetContentDirector();		
		player:AddDirector(director);		
		director:StartDirector(false);
		
		player:KickEvent(director, "noticeEvent", true);
		player:SetLoginDirector(director);		
		
		GetWorldManager():DoZoneChangeContent(player, contentArea, -5, 16.35, 6, 0.5, 16);		
		return;
	else
		player:EndEvent();
	end
end