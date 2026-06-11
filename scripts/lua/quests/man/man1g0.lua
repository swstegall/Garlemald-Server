-- ...

-- Actor Class Ids
MIOUNNE 					= 1000230;
-- ...
WEST_SHROUD_TRIGGER			= 1090067;
PUDGY_MOOGLE				= 1000328;
SHROUD_ECHO_TRIGGER			= 1090069; -- Replace 0 with actual actor class id

-- ...

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
		quest:SetENpc(WEST_SHROUD_TRIGGER, QFLAG_PUSH, false, true);
		quest:SetENpc(MIOUNNE);
	elseif (sequence == SEQ_035) then
		quest:SetENpc(PUDGY_MOOGLE, QFLAG_TALK);
		quest:SetENpc(SHROUD_ECHO_TRIGGER, QFLAG_PUSH, false, true);
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

function onPush(player, quest, actor)
	local sequence = quest:getSequence();
	local classId = actor:GetActorClassId();

	if (sequence == SEQ_030) then          -- Go to west shroud
		if (classId == WEST_SHROUD_TRIGGER) then
			callClientFunction(player, "processEvent060"); 
			quest:StartSequence(SEQ_035);
		end
	elseif (sequence == SEQ_035) then          -- Go to moogle
		if (classId == SHROUD_ECHO_TRIGGER) then
			callClientFunction(player, "processEvent070"); 
			quest:StartSequence(SEQ_040);
		end
	-- ...
end

function onTalk(player, quest, npc)
	-- ...
elseif (sequence == SEQ_065) then
	return MRKR_STEP12;          -- Replace MRKR_STEP13 with MRKR_STEP12
	-- ...