-- ...

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

-- ...

function onStateChange(player, quest, sequence)
	-- ...
	elseif (sequence == SEQ_100) then
		quest:SetENpc(ASSESSOR1);
		quest:SetENpc(ASSESSOR2);
		quest:SetENpc(PTAHJHA);
		quest:SetENpc(HALDBERK);
		quest:SetENpc(LILINA);
		quest:SetENpc(DODOROBA);
		quest:SetENpc(IVAN);
		quest:SetENpc(MERODAULYN);
		quest:SetENpc(COQUETTISH_PIRATE);
		quest:SetENpc(VOLUPTUOUS_PIRATE);
		quest:SetENpc(PEACOCKISH_PIRATE);
		quest:SetENpc(TRIGGER_ACN_LOWER, QFLAG_PUSH, false, true);
		quest:SetENpc(TRIGGER_ACN_UPPER, QFLAG_PUSH, false, true);
	end
	-- ...
end

function seq000_100_onTalk(player, quest, npc)
	if (npc == ASSESSOR1) then
		callClientFunction(player, "processEvent2000_10");
	elseif (npc == DODOROBA) then
		callClientFunction(player, "processEvent2000_11");
	end
end

function getJournalMapMarkerList(player, quest)
	local sequence = quest:getSequence();
	if (sequence == SEQ_050) then
		return {MRKR_TRIGGER_FSH};
	elseif (sequence == SEQ_080) then
		return {MRKR_TRIGGER_SEAFLD};
	elseif (sequence == SEQ_120) then
		return {MRKR_TRIGGER_ANC_LOWER};
	end
	return {};
end