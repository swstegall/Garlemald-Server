-- ...

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
EMERICK						= 1000185; -- Added Emerick actor
MERODAULYN					= 1000186; -- Added Merodaulyn actor

-- Quest Markers
JOURNAL_MARKER_SHIP_HOLD	= 1;
JOURNAL_MARKER_DUTY			= 2;
JOURNAL_MARKER_BADERON		= 3;

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
		-- Create duty instance
		local dutyInstance = CreateDutyInstance(player, EMERICK, MERODAULYN);
		quest:SetDutyInstance(dutyInstance);
		-- Start duty
		dutyInstance:Start();
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
			-- ...
	end
end

function onPush(player, quest, trigger)
	local sequence = quest:getSequence();
	local classId = trigger:GetActorClassId();

	if (sequence == SEQ_015) then
		if (classId == TRIGGER_DUTYSTART) then
			-- Start duty
			local dutyInstance = quest:GetDutyInstance();
			dutyInstance:Start();
			-- Wait for duty completion
			dutyInstance:WaitForCompletion();
			-- Get duty result
			local result = dutyInstance:GetResult();
			-- Process duty result
			if (result == 1) then
				-- Player won
				quest:StartSequence(SEQ_035);
			else
				-- Player lost
				-- Handle loss
			end
		end
	end
end

function getJournalMapMarkerList(player, quest)
	local sequence = quest:getSequence();
	local markers = {};

	if (sequence == SEQ_000) then
		table.insert(markers, JOURNAL_MARKER_SHIP_HOLD);
	elseif (sequence == SEQ_020) then
		table.insert(markers, JOURNAL_MARKER_DUTY);
	elseif (sequence == SEQ_035) then
		table.insert(markers, JOURNAL_MARKER_BADERON);
	end

	return markers;
end

function getJournalInformation(player, quest)
	local sequence = quest:getSequence();
	local progress = 0;

	if (sequence == SEQ_000) then
		progress = 10;
	elseif (sequence == SEQ_020) then
		progress = 50;
	elseif (sequence == SEQ_035) then
		progress = 100;
	end

	return progress, progress, progress;
end

-- Create duty instance function
function CreateDutyInstance(player, emerick, merodaulyn)
	-- Create duty instance
	local dutyInstance = DutyInstance();
	dutyInstance:SetPlayer(player);
	dutyInstance:SetEnemies({emerick, merodaulyn});
	return dutyInstance;
end