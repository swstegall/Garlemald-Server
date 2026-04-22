require ("global")
require ("quest")

--[[

Quest Script

Name: 	The House Always Wins
Code: 	Pgl200
Id: 	110060
Prereq: Level 20, Pugilist

]]

-- Sequence Numbers
SEQ_000	= 0;   -- Talk to Titinin.
SEQ_005	= 5;   -- Head to the GSM guild and talk to Esperaunce 3 times.
SEQ_010	= 10;  -- Warp into a duty; pickup coins.
SEQ_015	= 15;  -- Head to the ADV guild and talk to the Wise Miser. Journal Data 3: Set to 1 if talked to Titinin.
SEQ_025	= 25;  -- Head to the PGL guild entrence and talk to Lady Lewena. Journal Data 4: Set to 1 if talked to Titinin.
SEQ_030	= 30;  -- Head to GLA guild and warp into the duty and fight. Journal Data 5: Set to 1 once you win the fight.
SEQ_035	= 35;  -- Return to Titinin.

-- Actor Class Ids
ENPC_GAGARUNA 		= 1000862;
ENPC_TITININ 		= 1000934;
ENPC_NAIDA_ZAMAIDA	= 1000955;
ENPC_SINGLETON		= 1001445;
ENPC_TRIGGER_GSM	= 1090058;
ENPC_TRIGGER_PGL	= 1090042;
ENPC_PRIVAREA_EXIT	= 1290002;

-- PGL Actors
ENPC_MELISIE		= 1001009;
ENPC_GUNNULF		= 1001256;
ENPC_SHAMANI 		= 1001012;
ENPC_HALSTEIN		= 1001007;
ENPC_HEIBERT		= 1001257;
ENPC_IPAGHLO		= 1001260;

-- GSM Actors
ENPC_SULTRY_STRUMPET 	= 1000952;
ENPC_BEAUTEOUS_BEAUTY 	= 1000953;
ENPC_ESPERAUNCE			= 1000954;

-- Quest Markers
MRKR_TITININ		= 11006001;
MRKR_OBJECTIVE		= 11006002;
MRKR_ESPERAUNCE1	= 11006003;
MRKR_ESPERAUNCE2	= 11006004;
MRKR_NAIDA_ZAMADIA	= 11006005;
MRKR_OBJECTIVE2		= 11006006;
MRKR_SINGLETON		= 11006007;
MRKR_TITININ2		= 11006008;

-- Quest Details
ITEM_PLATINUM_LEDGER  = 11000134;
ITEM_KINGOFPLOTS_GIL  = 11000097;
ITEM_WISEMISER_GIL    = 11000098;
ITEM_LEWENA_GIL  	  = 11000099;
COUNTER_005 = 0;
COUNTER_015 = 1;
COUNTER_025 = 2;
COUNTER_030 = 3;

function onStart(player, quest)	
	quest:StartSequence(SEQ_000);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)	
	if (sequence == SEQ_ACCEPT) then
		quest:SetENpc(ENPC_GAGARUNA, QFLAG_TALK);
	elseif (sequence == SEQ_000) then
		quest:SetENpc(ENPC_TITININ, QFLAG_TALK);
		quest:SetENpc(ENPC_GAGARUNA);
		quest:SetENpc(ENPC_MELISIE);
		quest:SetENpc(ENPC_GUNNULF);
		quest:SetENpc(ENPC_SHAMANI);
		quest:SetENpc(ENPC_HALSTEIN);
		quest:SetENpc(ENPC_HEIBERT);
		quest:SetENpc(ENPC_IPAGHLO);
	elseif (sequence == SEQ_005) then
		quest:SetENpc(ENPC_TITININ);
		quest:SetENpc(ENPC_GAGARUNA);
		quest:SetENpc(ENPC_SULTRY_STRUMPET);
		quest:SetENpc(ENPC_BEAUTEOUS_BEAUTY);
		quest:SetENpc(ENPC_ESPERAUNCE, QFLAG_TALK);
		quest:SetENpc(ENPC_TRIGGER_GSM, QFLAG_NONE, false, true);
		quest:SetENpc(ENPC_PRIVAREA_EXIT, QFLAG_NONE, false, true);
	elseif (sequence == SEQ_010) then
		quest:SetENpc(ENPC_SULTRY_STRUMPET);
		quest:SetENpc(ENPC_BEAUTEOUS_BEAUTY);
		quest:SetENpc(ENPC_ESPERAUNCE);
	elseif (sequence == SEQ_015) then
		quest:SetENpc(ENPC_TITININ);
		quest:SetENpc(ENPC_NAIDA_ZAMAIDA, QFLAG_TALK);
		quest:SetENpc(ENPC_GAGARUNA);
	elseif (sequence == SEQ_025) then
		quest:SetENpc(ENPC_TITININ);
		quest:SetENpc(ENPC_GAGARUNA);
		quest:SetENpc(ENPC_TRIGGER_PGL, QFLAG_PUSH, false, true);
	elseif (sequence == SEQ_030) then
		quest:SetENpc(ENPC_TITININ);
		quest:SetENpc(ENPC_GAGARUNA);
		quest:SetENpc(ENPC_SINGLETON, QFLAG_TALK);
	elseif (sequence == SEQ_035) then
		quest:SetENpc(ENPC_TITININ, QFLAG_REWARD);
		quest:SetENpc(ENPC_GAGARUNA);
		quest:SetENpc(ENPC_MELISIE);
		quest:SetENpc(ENPC_GUNNULF);
		quest:SetENpc(ENPC_SHAMANI);
		quest:SetENpc(ENPC_HALSTEIN);
		quest:SetENpc(ENPC_HEIBERT);
		quest:SetENpc(ENPC_IPAGHLO);
	end				
end

function onTalk(player, quest, npc, eventName)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
	local data = quest:GetData();
    
	-- Offer the quest
	if (npcClassId == ENPC_GAGARUNA and seq == SEQ_ACCEPT) then
		local questAccepted = callClientFunction(player, "delegateEvent", player, quest, "processEventGagarunaStart");
		if (questAccepted == 1) then
			player:AcceptQuest(quest);
		end
		player:EndEvent();
		return;	
	-- Quest Progress
	elseif (seq == SEQ_000) then
        if (npcClassId == ENPC_TITININ) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
			player:SendGameMessage(GetWorldMaster(), 25246, MESSAGE_TYPE_SYSTEM, ITEM_PLATINUM_LEDGER, 1);
			quest:StartSequence(SEQ_005);
		elseif (npcClassId == ENPC_GAGARUNA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005_2");
		elseif (npcClassId == ENPC_IPAGHLO) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005_3");
		elseif (npcClassId == ENPC_HALSTEIN) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005_4");
		elseif (npcClassId == ENPC_MELISIE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005_5");
		elseif (npcClassId == ENPC_HEIBERT) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005_6");
		elseif (npcClassId == ENPC_GUNNULF) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005_7");
		elseif (npcClassId == ENPC_SHAMANI) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent005_8");
		end	
	elseif (seq == SEQ_005) then
		if (npcClassId == ENPC_ESPERAUNCE) then
			local talkCount = data:IncCounter(COUNTER_005);
			if (talkCount == 1) then
				player:SendGameMessage(quest, 117, MESSAGE_TYPE_SYSTEM);
			elseif (talkCount == 2) then
				player:SendGameMessage(quest, 118, MESSAGE_TYPE_SYSTEM);
			elseif (talkCount >= 3) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent020");
				quest:StartSequence(SEQ_010); -- Temp until Duty is finished. Should go to a duty here.
			end
		elseif (npcClassId == ENPC_TITININ) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010_2");
		elseif (npcClassId == ENPC_GAGARUNA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010_3");
		elseif (npcClassId == ENPC_SULTRY_STRUMPET) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010_4");
		elseif (npcClassId == ENPC_BEAUTEOUS_BEAUTY) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010_5");
		end
	elseif (seq == SEQ_010) then 
		if (npcClassId == ENPC_ESPERAUNCE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020_2");
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030");
			quest:StartSequence(SEQ_015); -- Temp until Duty is finished.
			player:SendGameMessage(GetWorldMaster(), 25246, MESSAGE_TYPE_SYSTEM, ITEM_KINGOFPLOTS_GIL, 1);
			player:EndEvent();			
			GetWorldManager():WarpToPublicArea(player);
			return;
		elseif (npcClassId == ENPC_SULTRY_STRUMPET) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020_3");
		elseif (npcClassId == ENPC_BEAUTEOUS_BEAUTY) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020_4");
		end
	elseif (seq == SEQ_015) then
		if (npcClassId == ENPC_NAIDA_ZAMAIDA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040");
			quest:StartSequence(SEQ_025);
			player:SendGameMessage(GetWorldMaster(), 25246, MESSAGE_TYPE_SYSTEM, ITEM_WISEMISER_GIL, 1);
		elseif (npcClassId == ENPC_TITININ) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030_2");
			data:SetCounter(COUNTER_015, 1);
		elseif (npcClassId == ENPC_GAGARUNA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030_3");
		end
	elseif (seq == SEQ_025) then
		if (npcClassId == ENPC_PUSH_PGL) then
		elseif (npcClassId == ENPC_TITININ) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040_2");
			data:SetCounter(COUNTER_025, 1);
		elseif (npcClassId == ENPC_GAGARUNA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040_3");
		end
	elseif (seq == SEQ_030) then
		if (npcClassId == ENPC_SINGLETON) then
			local enterDuty = callClientFunction(player, "delegateEvent", player, quest, "processEvent050_4");
			if (enterDuty == 1) then
				--Enter duty at this point....
				callClientFunction(player, "delegateEvent", player, quest, "processEvent060");
				data:SetCounter(COUNTER_030, 1);
				quest:StartSequence(SEQ_035); -- Temp until Duty is finished.
				player:SendGameMessage(GetWorldMaster(), 25246, MESSAGE_TYPE_SYSTEM, ITEM_LEWENA_GIL, 1);
				GetWorldManager():DoZoneChange(player, 209, nil, 0, 0x2, -192.0, 194.5, 193.785, 3.0);
			end
		elseif (npcClassId == ENPC_TITININ) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent050_2");
		elseif (npcClassId == ENPC_GAGARUNA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent050_3");
		end
	elseif (seq == SEQ_035) then
		if (npcClassId == ENPC_TITININ) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent070");			
            callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 200, 2)
			player:CompleteQuest(quest);
		elseif (npcClassId == ENPC_GAGARUNA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_2");
		elseif (npcClassId == ENPC_IPAGHLO) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_3");
		elseif (npcClassId == ENPC_HALSTEIN) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_4");
		elseif (npcClassId == ENPC_MELISIE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_5");
		elseif (npcClassId == ENPC_HEIBERT) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_6");
		elseif (npcClassId == ENPC_GUNNULF) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_7");
		elseif (npcClassId == ENPC_SHAMANI) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060_8");
		end
	end
	
	quest:UpdateENPCs();	
	player:EndEvent();
end

function onPush(player, quest, npc)
	local npcClassId = npc.GetActorClassId();
	local seq = quest:GetSequence();
	local data = quest:GetData();
	
	if (seq == SEQ_005) then
		player:EndEvent();
		if (npcClassId == ENPC_TRIGGER_GSM) then
			GetWorldManager():WarpToPrivateArea(player, "PrivateAreaMasterPast", 5);
		elseif (npcClassId == ENPC_PRIVAREA_EXIT) then
			GetWorldManager():WarpToPublicArea(player);
		end
	elseif (seq == SEQ_025) then
		if (npcClassId == ENPC_TRIGGER_PGL) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent050");
			quest:StartSequence(SEQ_030);
			player:EndEvent();
		end
	end
	
end

function getJournalInformation(player, quest)
	local data = quest:GetData();
	return 0, data:GetCounter(COUNTER_015), data:GetCounter(COUNTER_025), data:GetCounter(COUNTER_030);
end

function getJournalMapMarkerList(player, quest)
    local seq = quest:getSequence();
	local data = quest:GetData();	
    
    if (seq == SEQ_000) then
		return MRKR_TITININ;
    elseif (seq == SEQ_005) then
        return MRKR_ESPERAUNCE1;
	elseif (seq == SEQ_010) then
		return MRKR_OBJECTIVE;
	elseif (seq == SEQ_015) then		
		if (data:GetCounter(COUNTER_015) == 1) then
			return MRKR_NAIDA_ZAMADIA;
		end
	elseif (seq == SEQ_025) then
		if (data:GetCounter(COUNTER_025) == 1) then
			return MRKR_OBJECTIVE2;
		end
	elseif (seq == SEQ_030) then
		return MRKR_SINGLETON;		
	elseif (seq == SEQ_035) then
		return MRKR_TITININ2;
    end
	
	return;
end