require ("global")
require ("quest")

--[[

Quest Script

Name: 	Never the Twain Shall Meet
Code: 	Man2l0
Id: 	110004
Prereq: Legends Adrift (Man1l0 - 110003)

]]

-- Sequence Numbers
SEQ_000	= 0;  	-- Talk to Captain Hob.
SEQ_010	= 10;  	-- Ship instance, enter the hold.
SEQ_015	= 15;  	-- Exit the hold, go back upstairs.
SEQ_020	= 20;  	-- Duty, fight Emerick and Merodaulyn
SEQ_035	= 35;  	-- Head to Baderon and chat.
SEQ_037	= 37;  	-- Head to outcrop in La Noscea.
SEQ_040	= 40;  	-- Talk to Baderon on the Link Pearl
SEQ_042	= 42;  	-- Enter and push at the MSK guild.
SEQ_045	= 45;  	-- Talk to Isaudorel
SEQ_050	= 50;  	-- Head to God's Grip push, talk with Blackburn.
SEQ_055	= 55;  	-- Continue to the other push with Y'shtola in the subecho.
SEQ_060	= 60;  	-- Unused? Talks about spying Stahlmann, Emerick, and Merod scheming.
SEQ_065	= 65;  	-- Unused? Talks about the meteor shower and the Ascian stealing the key.
SEQ_070	= 70;  	-- Unused? Talks about heading to Ul'dah

-- Quest Actors
BADERON 					= 1000137;
YSHTOLA 					= 1000001;
HOB							= 1000151;
ISAUDOREL					= 1000152;
BARRACUDA_KNIGHT1			= 1000183;
BARRACUDA_KNIGHT2			= 1000184;
TRIGGER_DOCKS				= 1090386;
EVENTDOOR_SHIP1				= 1090098;
EVENTDOOR_SHIP2				= 1090099;
TRIGGER_DUTYSTART			= 1090085;
TRIGGER_MSK					= 1090003;
TRIGGER_SEAFLD1				= 1090082;
TRIGGER_SEAFLD2				= 1090086;
TRIGGER_SEAFLD3				= 1090087;

-- Quest Markers

-- Msg packs for the Npc LS
NPCLS_MSGS = {
	{40, 41} 	-- SEQ_040
};

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)
	local data = quest:GetData();

	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(BADERON, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
		quest:SetENpc(TRIGGER_DOCKS, QFLAG_PUSH, false, true);
		quest:SetENpc(HOB, QFLAG_TALK);
		quest:SetENpc(BADERON);
	elseif (sequence == SEQ_010) then
		quest:SetENpc(TRIGGER_DOCKS, QFLAG_NONE, false, true);
		quest:SetENpc(HOB);
		quest:SetENpc(BARRACUDA_KNIGHT1);
		quest:SetENpc(BARRACUDA_KNIGHT2);
		quest:SetENpc(EVENTDOOR_SHIP1, QFLAG_PUSH, false, true);
	elseif (sequence == SEQ_015) then
		quest:SetENpc(TRIGGER_DOCKS, QFLAG_NONE, false, true);
		quest:SetENpc(HOB);
		quest:SetENpc(BARRACUDA_KNIGHT1);
		quest:SetENpc(BARRACUDA_KNIGHT2);
		quest:SetENpc(EVENTDOOR_SHIP2, QFLAG_PUSH, false, true);
		quest:SetENpc(TRIGGER_DUTYSTART, QFLAG_PUSH, false, true);
	elseif (sequence == SEQ_020) then
		-- DUTY HAPPENS HERE
	elseif (sequence == SEQ_035) then
		quest:SetENpc(BADERON, QFLAG_TALK);
	elseif (sequence == SEQ_037) then
		quest:SetENpc(TRIGGER_SEAFLD1, QFLAG_PUSH, false, true);
		quest:SetENpc(BADERON);
	elseif (sequence == SEQ_042) then
		quest:SetENpc(TRIGGER_MSK, QFLAG_PUSH, false, true);
		quest:SetENpc(BADERON);
	elseif (sequence == SEQ_045) then
		quest:SetENpc(ISAUDOREL, QFLAG_TALK);
	elseif (sequence == SEQ_050) then
		quest:SetENpc(TRIGGER_SEAFLD2, QFLAG_PUSH, false, true);
	elseif (sequence == SEQ_055) then
		quest:SetENpc(TRIGGER_SEAFLD3, QFLAG_PUSH, false, true);
		quest:SetENpc(YSHTOLA);
	end	
	
end

function onTalk(player, quest, npc)
	local sequence = quest:getSequence();
	local classId = npc:GetActorClassId();

	if (sequence == SEQ_ACCEPT) then
		if (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent000");
			player:EndEvent();
			player:AcceptQuest(quest, true);
			return;
		end
	elseif (sequence == SEQ_000) then		
		if (classId == HOB) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");			
			quest:StartSequence(SEQ_010);
			player:EndEvent();
			GetWorldManager():DoZoneChange(player, 192, "PrivateAreaMasterPast", 0, 0, 1832.243, 16.352, 1834.965, 1.584);
			return;
		elseif (classId == BADERON) then
			if (npc.CurrentArea.IsPrivate()) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent000_2");
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent010_3");
			end
		end
	elseif (sequence >= SEQ_010 and sequence <= SEQ_020) then
		if (onTalk_shipSequences(player, quest, npc, classId, sequence) == 1) then
			return;
		end
	elseif (sequence == SEQ_035) then
		if (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent050");
			quest:StartSequence(SEQ_037);
		end		
	elseif (sequence == SEQ_037) then
		if (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent050_2");
		end	
	elseif (sequence == SEQ_042) then
		if (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_2");
		end
	elseif (sequence == SEQ_045) then
		if (classId == ISAUDOREL) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent075");
			quest:StartSequence(SEQ_050);
		end
	elseif (sequence == SEQ_055) then
		if (classId == YSHTOLA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent080_2");
		end
	end	
	
	player:EndEvent();
	quest:UpdateENPCs();
end

function onTalk_shipSequences(player, quest, npc, classId, sequence)
	if (classId == HOB) then
		if (npc.CurrentArea.ZoneId == 230 and npc.CurrentArea.IsPrivate()) then
			local returnToShip = callClientFunction(player, "delegateEvent", player, quest, "processEvent010_2");
			if (returnToShip == 1) then
				player:EndEvent();				
				if (sequence == SEQ_015 or sequence == SEQ_020) then
					GetWorldManager():DoZoneChange(player, 192, "PrivateAreaMasterPast", 0, 0, 1828.785, 11.852, 1829.20, -1.675);
				else
					GetWorldManager():DoZoneChange(player, 192, "PrivateAreaMasterPast", 0, 0, 1832.243, 16.352, 1834.965, 1.584);
				end
				return 1;
			end
		elseif (npc.CurrentArea.ZoneId == 192 and npc.CurrentArea.IsPrivate()) then
			local returnToPublic = callClientFunction(player, "delegateEvent", player, quest, "processEvent011_2");
			if (returnToPublic == 1) then
				player:EndEvent();
				GetWorldManager():DoZoneChange(player, 230, nil, 0, 0, -639.325, 1, 403.967, 1.655);
				return 1;
			end
		elseif (npc.CurrentArea.IsPublic()) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010_3");
		end
	elseif (classId == BARRACUDA_KNIGHT1) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent011_3");
	elseif (classId == BARRACUDA_KNIGHT2) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent011_4");
	end		
end

function onPush(player, quest, npc)
	local data = quest:GetData();
	local sequence = quest:getSequence();
	local classId = npc:GetActorClassId();
	
	if (sequence >= SEQ_000 and sequence <= SEQ_020) then
		if (classId == TRIGGER_DOCKS) then
			player:EndEvent();
			GetWorldManager():WarpToPrivateArea(player, "PrivateAreaMasterPast", 8, -631.93, 2, 391.75, -0.05);
			return;
		end
		if (sequence == SEQ_010) then
			if (classId == EVENTDOOR_SHIP1) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent012");
				quest:StartSequence(SEQ_015);
				player:EndEvent();
				GetWorldManager():WarpToPrivateArea(player, "PrivateAreaMasterPast", 1, 1823.579, -61.65, 1816.102, 2.42);
				return;
			end
		elseif (sequence == SEQ_015) then
			if (classId == EVENTDOOR_SHIP2) then
				player:EndEvent();
				GetWorldManager():WarpToPrivateArea(player, "PrivateAreaMasterPast", 0, 1821.675, 10.352, 1814.964, 2.288);
				return;
			elseif (classId == TRIGGER_DUTYSTART) then
				local result = callClientFunction(player, "delegateEvent", player, quest, "contentsJoinAskInBasaClass");
				if (result == 1) then
					-- DO COMBAT DUTY HERE				
					-- For now just skip the sequence
					quest:StartSequence(SEQ_020);
					math.randomseed(os.time());
					local randomVal = math.random(1, 2); -- Randomize the winner for now
					callClientFunction(player, "delegateEvent", player, quest, "processEvent020", randomVal);
					player:EndEvent();
					quest:StartSequence(SEQ_035);
					GetWorldManager():DoZoneChange(player, 230, nil, 0, 0, -639.325, 1, 403.967, 1.655);
					return;
				end
				player:EndEvent();
			end
		end
	elseif (sequence == SEQ_037 and classId == TRIGGER_SEAFLD1) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent060");
		quest:NewNpcLsMsg(1);
		quest:StartSequence(SEQ_040);
	elseif (sequence == SEQ_042 and classId == TRIGGER_MSK) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent070");
		quest:StartSequence(SEQ_045);
	elseif (sequence == SEQ_050 and classId == TRIGGER_SEAFLD2) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent080");
		quest:StartSequence(SEQ_055);
		GetWorldManager():DoZoneChange(player, 128, "PrivateAreaMasterPast", 3, 0, 198.314, 25.928, 1186.126, 1.6);
	elseif (sequence == SEQ_055 and classId == TRIGGER_SEAFLD3) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent081");
		GetWorldManager():DoZoneChange(player, 133, nil, 0, 0, -435.501, 40, 202.698, -2.152);
	end	
	
	player:EndEvent();
	quest:UpdateENPCs();
end

function onNotice(player, quest, target)
	callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 300, 1, 1, 2);
	player:CompleteQuest(quest);
    callClientFunction(player, "delegateEvent", player, quest, "processEvent081_2", 1);
    player:EndEvent();
    quest:UpdateENPCs();
end

function onNpcLS(player, quest, from, msgStep)
	local sequence = quest:getSequence();
	local msgPack;

	if (from == 1) then
		-- Get the right msg pack
		if (sequence == SEQ_040 or sequence == SEQ_042) then
			msgPack = 1;
		end	
				
		-- Quick way to handle all msgs nicely.
		player:SendGameMessageLocalizedDisplayName(quest, NPCLS_MSGS[msgPack][msgStep], MESSAGE_TYPE_NPC_LINKSHELL, 1000015);
		if (msgStep >= #NPCLS_MSGS[msgPack]) then
			quest:EndOfNpcLsMsgs();
		else
			quest:ReadNpcLsMsg();
		end
		
		-- Handle anything else
		if (sequence == SEQ_040) then
			quest:StartSequenceForNpcLs(SEQ_042);
		end
	end
	
	player:EndEvent();
end

function getJournalInformation(player, quest)
	return 40, 40, 40;
end

function getJournalMapMarkerList(player, quest)
	local sequence = quest:getSequence();
	
end