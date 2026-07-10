require ("global")
require ("quest")

--[[

Quest Script

Name:   Court in the Sands
Code:   Man0u1
Id:     110010
Prereq: Flowers for All (Man0u0 - 110009)

Full port for Garlemald-Server #53. The retail flow, sequence numbers,
text ids, casts, and coordinates are evidence-backed — see
docs/implementation/quest_man0u1_court_in_the_sands.md for the source
map (retail OCR playthroughs, Slerp Lederp's dev captures, the decoded
client script + csv sheets, and the pmeteor phase-table header this
skeleton shipped with).

Vid refs -
https://www.youtube.com/watch?v=WNRLrwZ3BJY (retail part 1)
https://www.youtube.com/watch?v=eZgcq-FMpfw (retail part 2)
https://www.youtube.com/watch?v=Jcv9I2Bk46w (coliseum fight)
https://www.youtube.com/watch?v=sTXkNoVNlRc (Slerp Lederp dev demo)

]]

-- Sequence Numbers
SEQ_000 = 0;   -- Ul'dah Adventurer's Guild (Quicksand intro instance)
SEQ_005 = 5;   -- Run to Camp Black Brush & Attune
SEQ_010 = 10;  -- Return to the Guild
SEQ_012 = 12;  -- Speak to Momodi again (guild schooling + Coliseum Pass)
SEQ_015 = 15;  -- Visiting guilds (GSM Eshtaime's, GLD Coliseum) — parallel counters
SEQ_045 = 45;  -- Go to Amajina & Sons Mineral Concern (Linette)
SEQ_050 = 50;  -- Miner's Guild Instance #1 — F'lhaminn teaches the six emotes
SEQ_057 = 57;  -- Calm the two miners (Maddened: Beckon; Manic: Soothe, Furious, Laugh)
SEQ_058 = 58;  -- Miner's Guild Instance #2 — Corguevais / Linette assignment
SEQ_060 = 60;  -- Meet F'lhaminn at the Gate of Nald; the escort duty runs here
SEQ_065 = 65;  -- Camp Black Brush — the Ascilia echo + the camp leader
SEQ_070 = 70;  -- Return to Ul'dah — Momodi gossip
SEQ_075 = 75;  -- F'lhaminn at the Concern stage — escort payment
SEQ_080 = 80;  -- Frondale's Phrontistery — Nogeloix
SEQ_085 = 85;  -- Master Faustigeant — sickroom survey assignment
SEQ_090 = 90;  -- Survey the sickrooms (ward instance) → the Warburton echo
SEQ_095 = 95;  -- Report to Faustigeant (no pay)
SEQ_100 = 100; -- Momodi's linkpearl call
SEQ_105 = 105; -- Turn-in at Momodi
SEQ_110 = 110; -- declared upstream, unused — completion fires at SEQ_105
               -- (Slerp's capture shows Sequence: 105 as the last
               -- pre-completion state; man0g1 completes on its final
               -- declared sequence the same way)

-- Actor Class Ids — SEQ_000 Quicksand intro cast (upstream skeleton)
OVERCOMPETITIVE_ADVENTURER  = 1000807;
MOMODI                      = 1000841;
OTOPA_POTTOPA               = 1000864;
UNDAUNTED_ADVENTURER        = 1000936;
GREEDY_MERCHANT             = 1000937;
LIONHEARTED_ADVENTURER      = 1000938;
SPRY_SALESMAN               = 1000939;
UPBEAT_ADVENTURER           = 1000940;
SEEMINGLY_CALM_ADVENTURER   = 1000941;
-- Class 1000948 is a bare gamedata row whose display_name_id is
-- 1000010 — i.e. it borrows the canonical Thancred class's name (the
-- upstream "-- 1000010" comment was pointing at exactly that; bahamut
-- gamedata_actor_class.sql:964). It is the Quicksand-scene Thancred.
THANCRED                    = 1000948;

-- SEQ_015 — Eshtaime's Lapidaries (GSM leg)
ELECOTTE                    = 1000950;
HNAUFRID                    = 1000635;
PAMISOLAUX                  = 1600040;

-- SEQ_015 — the Coliseum (GLD leg). Public entrance + lobby/echo PA
-- cast (display-name → class resolutions via bahamut
-- gamedata_actor_class.sql; see the design doc).
YOYOBINA                    = 1000927;
LULUTSU                     = 1000863;
PAPAWA                      = 1000962;
ABYLGO_HAMYLGO              = 1000965;
FRUHYBOLG                   = 1000964;
SWERDAHRM                   = 1000967;
GREINFARR                   = 1000039;
NIELLEFRESNE_GLD            = 1000037; -- echo scene; base Niellefresne class
ARENA_TRIGGER               = 1090283; -- pushWithCircle at the arena gate (Slerp's ">Pushed GLD!" class)

-- SEQ_045..058 — Amajina & Sons Mineral Concern (public + scene PA)
LINETTE                     = 1000861;
SHILGEN                     = 1000637;
TYAGO_MOUI                  = 1001203;
NORTMOEN                    = 1600042;
NITTMA_GUTTMA               = 1001286;
CORGUEVAIS_SCENE            = 1001054; -- beaten-in-the-mines scene variant (header: 1000043/1001054)
CORGUEVAIS                  = 1000043; -- present-day (camp echo)
FLHAMINN_SCENE              = 1000842; -- the teaching/instance F'lhaminn (header roster)
FLHAMINN                    = 1000038; -- base populace (gate muster + Concern stage)
MANIC_MINER                 = 1001283;
MADDENED_MINER              = 1001284;
MAUDLIN_MINER               = 1001287;
MOCKING_MINER               = 1001288;
MONITORING_MINER            = 1001289;
DISPLEASED_DANCER           = 1001290;
MUSCULAR_MINER              = 1000690;
CLOSE_FISTED_WOMAN          = 1000981;
ASTONISHED_ADVENTURER       = 1000895;
POPOKKULI                   = 1000857;
SESERUKKA                   = 1000858;

-- SEQ_060 — Gate of Nald (zone 170)
GATE_OF_NALD_TRIGGER        = 1090285; -- net-new pushWithCircle class (seed/095)

-- SEQ_065 — Camp Black Brush echo cast (public zone 170)
ASCILIA_CAMP                = 1001515; -- camp-scene variant (1000042 is man0u0's parade Ascilia)
THANCRED_CAMP               = 1000185;
UNTIDY_OUTLANDER            = 1000808;
MALNOURISHED_MIDLANDER      = 1000809;
HELPLESS_HYUR               = 1000817; -- the camp leader (retail placeholder name)

-- SEQ_080..100 — Frondale's Phrontistery
NOGELOIX                    = 1000597;
KYLENE                      = 1000634;
FAUSTIGEANT                 = 1000852;
WARD_DOOR                   = 1090119; -- ObjectEventDoor (Slerp's SEQ 90/95/100 push class)
SLOW_WITTED_MINER           = 1000689;
QUALMISH_MINER              = 1000699;
WORRISOME_ASSISTANT         = 1001207;
WELL_WASHED_LEECH           = 1001210;

-- Quest Markers — client quest_marker.csv 110010xx block. The sheet's
-- x/z pairs are literal world coordinates (11001009 == Linette's spawn
-- row, 11001018 == Nogeloix's), which is also where the seed/094 scene
-- casts take their positions from.
MRKR_MOMODI             = 11001001;
MRKR_CAMP_BLACK_BRUSH   = 11001002;
MRKR_ESHTAIME           = 11001005;
MRKR_COLISEUM           = 11001006;
MRKR_LINETTE            = 11001009;
MRKR_FLHAMINN_TEACH     = 11001010;
MRKR_MANIC_MINER        = 11001011;
MRKR_MADDENED_MINER     = 11001012;
MRKR_GATE_OF_NALD       = 11001014;
MRKR_CAMP_LEADER        = 11001015;
MRKR_CONCERN_STAGE      = 11001017;
MRKR_PHRONTISTERY       = 11001018;
MRKR_WARD_DOOR          = 11001019;

-- Quest Items
ITEM_VELODYNA_COSMOS = 11000089; -- mozk-tabetai + man0u1.csv @SHEET refs
ITEM_COLISEUM_PASS   = 11000126; -- granted at processEvent017 (toast 25117)

-- Quest Counters
CNTR_SEQ15_GSM   = 0; -- 0 = Eshtaime's pending, 1 = flower sold
CNTR_SEQ15_GLD   = 1; -- 0 pending, 1 recruit CS seen (in lobby), 2 pass
                      -- checked (arena trigger armed), 3 fight handed
                      -- off, 4 fight done (echo PA), 5 leg complete
CNTR_EMOTE_TEACH = 2; -- SEQ_050: teach steps completed (0..6)
CNTR_MANIC_CALM  = 3; -- SEQ_057: Manic Miner sequence progress (0..3)

-- Quest Flags
-- One-shot handoff latch for the arena content (the man0l1/man0g1
-- FLAG_ESCORT_HANDOFF shape, adapted to a counter-state machine): set
-- by startMan0u1Coliseum right before the content warp. A later
-- onStateChange arrival at SEQ_015 with GLD == 3 and the flag CLEAR is
-- the relog re-arm for a player whose arena content died with the
-- session — roll the counter back to 2 so the arena trigger re-arms.
FLAG_ARENA_HANDOFF   = 0;
FLAG_CALM_MADDENED   = 1; -- SEQ_057: Maddened Miner becalmed (Beckon)
FLAG_CAMP_ECHO_SEEN  = 2; -- SEQ_065: the Ascilia echo CS has played

-- The six taught emotes (EmoteStandardCommand 100-range ids — the
-- client-side F'lhaminn demo schedulers encode exactly these:
-- 0x5000000 | ((id-100) << 12)). DoEmote takes (id-100) and the motion
-- message id 21000+(id-101)*10+11; formula cross-checked against four
-- sibling data points (105→21041, 107→21061, 118→21171, 116→21151).
-- Laugh/Deny/Upset/Soothe motion ids are formula-derived — flagged in
-- the design doc as a live-test checkpoint.
EMOTE_STEPS = {
	{ eventName = "emoteDefault1", doEmoteId = 3,  motionMsg = 21021 }, -- Furious  (103)
	{ eventName = "emoteDefault2", doEmoteId = 8,  motionMsg = 21071 }, -- Beckon   (108)
	{ eventName = "emoteDefault3", doEmoteId = 21, motionMsg = 21201 }, -- Laugh    (121)
	{ eventName = "emoteDefault4", doEmoteId = 25, motionMsg = 21241 }, -- Deny     (125)
	{ eventName = "emoteDefault5", doEmoteId = 40, motionMsg = 21391 }, -- Upset    (140)
	{ eventName = "emoteDefault6", doEmoteId = 35, motionMsg = 21341 }, -- Soothe   (135)
};
-- Manic Miner wants Soothe → Furious → Laugh, in that order (the
-- pmeteor header's "Emotes 135, 103, 121 @ Manic Miner").
MANIC_SEQUENCE = { 6, 1, 3 };
-- Maddened Miner wants Beckon (header: "Emotes 108 @ Maddened Miner").
MADDENED_STEP = 2;

-- NpcLS message packs — man0u1 QUEST-sheet text ids, sender Momodi
-- (display 1500014). Pack 1 = the post-guilds Amajina recruiting call
-- (verbatim in the retail capture), pack 2 = the cave-in news, pack 3
-- = "come and see me at the Quicksand".
NPCLS_MSGS = {
	{301, 302, 303},
	{184, 185, 186},
	{307, 308}
};
MOMODI_DISPLAY_ID = 1500014;

function onStart(player, quest)
    quest:StartSequence(SEQ_000);

    -- Immediately move to the Adventurer's Guild private area
	callClientFunction(player, "delegateEvent", player, quest, "processEventMomodiStart");
	-- pmeteor's script order: arrival texts BEFORE the warp (texts
	-- inside the Now-Loading gap rode every crashing handoff — see
	-- man0l1), then the event closer, then the warp (0x0131 ahead of
	-- the 0x00E2 latch is retail's invariant ordering).
	player:SendGameMessage(quest, 329, 0x20);
	player:SendGameMessage(quest, 330, 0x20);
	player:EndEvent();
    GetWorldManager():DoZoneChange(player, 175, "PrivateAreaMasterPast", 4, 15, -75.242, 195.009, 74.572, -0.046);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)
	local data = quest:GetData();

    if (sequence == SEQ_000) then
        quest:SetENpc(MOMODI, QFLAG_TALK);
        quest:SetENpc(OTOPA_POTTOPA);
        quest:SetENpc(UNDAUNTED_ADVENTURER);
        quest:SetENpc(GREEDY_MERCHANT);
        quest:SetENpc(SPRY_SALESMAN);
        quest:SetENpc(LIONHEARTED_ADVENTURER);
        quest:SetENpc(UPBEAT_ADVENTURER);
        quest:SetENpc(SEEMINGLY_CALM_ADVENTURER);
        quest:SetENpc(OVERCOMPETITIVE_ADVENTURER);
        quest:SetENpc(THANCRED);
    elseif (sequence == SEQ_005) then
        quest:SetENpc(MOMODI);
	elseif (sequence == SEQ_010) then
		quest:SetENpc(MOMODI, QFLAG_TALK);
	elseif (sequence == SEQ_012) then
		quest:SetENpc(MOMODI, QFLAG_TALK);
	elseif (sequence == SEQ_015) then
		local subseqGSM = data:GetCounter(CNTR_SEQ15_GSM);
		local subseqGLD = data:GetCounter(CNTR_SEQ15_GLD);
		-- Relog rescue for a player whose arena content died with the
		-- session (content instances don't persist): the handoff latch
		-- is only SET on the live start path, so a re-arm arrival at
		-- GLD == 3 with it CLEAR rolls back to 2 and the arena trigger
		-- re-arms for a retry. (man0l1 FLAG_ESCORT_HANDOFF pattern.)
		if (subseqGLD == 3) then
			if (data:GetFlag(FLAG_ARENA_HANDOFF)) then
				data:ClearFlag(FLAG_ARENA_HANDOFF);
			else
				data:SetCounter(CNTR_SEQ15_GLD, 2);
				subseqGLD = 2;
			end
		end
		-- Momodi is always talkable for the 017_2/3/4 reminders.
		quest:SetENpc(MOMODI);
		-- GSM leg — Eshtaime's Lapidaries (public 209 half).
		quest:SetENpc(ELECOTTE, (subseqGSM == 0) and QFLAG_TALK or QFLAG_NONE);
		quest:SetENpc(HNAUFRID);
		quest:SetENpc(PAMISOLAUX);
		-- GLD leg — Coliseum entrance (public), lobby + echo (PA 209/5).
		quest:SetENpc(YOYOBINA, (subseqGLD == 0) and QFLAG_TALK or QFLAG_NONE);
		quest:SetENpc(LULUTSU, (subseqGLD == 1) and QFLAG_TALK or QFLAG_NONE);
		quest:SetENpc(ARENA_TRIGGER, (subseqGLD == 2) and QFLAG_PUSH or QFLAG_NONE, false, (subseqGLD == 2));
		quest:SetENpc(PAPAWA);
		quest:SetENpc(ABYLGO_HAMYLGO);
		quest:SetENpc(FRUHYBOLG);
		quest:SetENpc(SWERDAHRM);
		quest:SetENpc(GREINFARR, (subseqGLD == 4) and QFLAG_TALK or QFLAG_NONE);
		quest:SetENpc(NIELLEFRESNE_GLD);
	elseif (sequence == SEQ_045) then
		quest:SetENpc(LINETTE, QFLAG_TALK);
		quest:SetENpc(SHILGEN);
		quest:SetENpc(TYAGO_MOUI);
		quest:SetENpc(NORTMOEN);
	elseif (sequence == SEQ_050) then
		-- Miner instance #1 (PA 209/5, Concern interior) — the teach
		-- phase. F'lhaminn is talk + emote enabled; the whole crowd is
		-- talkable per the pmeteor header's event table.
		quest:SetENpc(FLHAMINN_SCENE, QFLAG_TALK, true, false, true);
		seq050_common_cast(quest);
	elseif (sequence == SEQ_057) then
		-- The calming phase: each miner is emote-enabled until becalmed.
		local manicDone    = data:GetCounter(CNTR_MANIC_CALM) >= #MANIC_SEQUENCE;
		local maddenedDone = data:GetFlag(FLAG_CALM_MADDENED);
		quest:SetENpc(FLHAMINN_SCENE, QFLAG_NONE, true, false, true);
		quest:SetENpc(MANIC_MINER, (not manicDone) and QFLAG_TALK or QFLAG_NONE, true, false, not manicDone);
		quest:SetENpc(MADDENED_MINER, (not maddenedDone) and QFLAG_TALK or QFLAG_NONE, true, false, not maddenedDone);
		seq050_common_cast(quest, true);
	elseif (sequence == SEQ_058) then
		-- Miner instance #2 (same PA, reloaded) — Corguevais recovered.
		quest:SetENpc(CORGUEVAIS_SCENE, QFLAG_TALK);
		quest:SetENpc(FLHAMINN_SCENE);
		quest:SetENpc(LINETTE);
		quest:SetENpc(POPOKKULI);
		quest:SetENpc(SESERUKKA);
		quest:SetENpc(NITTMA_GUTTMA);
		quest:SetENpc(MAUDLIN_MINER);
		quest:SetENpc(MONITORING_MINER);
		quest:SetENpc(MUSCULAR_MINER);
	elseif (sequence == SEQ_060) then
		-- The Gate of Nald muster (zone 170, north of the city).
		-- F'lhaminn waits at the gate; the trigger starts the escort.
		-- The escort itself runs at THIS sequence: if the content dies
		-- with the session, this re-arm puts the player back at the
		-- gate with the trigger live — retry, no rollback state needed.
		quest:SetENpc(FLHAMINN, QFLAG_TALK);
		quest:SetENpc(GATE_OF_NALD_TRIGGER, QFLAG_PUSH, false, true);
	elseif (sequence == SEQ_065) then
		-- Camp Black Brush, public zone 170 (no client-provable PA for
		-- wil0Fld01 — design-doc checkpoint 4). The Ascilia echo plays
		-- from her talk; the leader beat unlocks after it.
		local echoSeen = data:GetFlag(FLAG_CAMP_ECHO_SEEN);
		quest:SetENpc(ASCILIA_CAMP, (not echoSeen) and QFLAG_TALK or QFLAG_NONE);
		quest:SetENpc(CORGUEVAIS);
		quest:SetENpc(THANCRED_CAMP);
		quest:SetENpc(UNTIDY_OUTLANDER);
		quest:SetENpc(MALNOURISHED_MIDLANDER);
		quest:SetENpc(HELPLESS_HYUR, echoSeen and QFLAG_TALK or QFLAG_NONE);
	elseif (sequence == SEQ_070) then
		quest:SetENpc(MOMODI, QFLAG_TALK);
	elseif (sequence == SEQ_075) then
		quest:SetENpc(FLHAMINN, QFLAG_TALK);
		quest:SetENpc(LINETTE);
	elseif (sequence == SEQ_080) then
		quest:SetENpc(NOGELOIX, QFLAG_TALK);
		quest:SetENpc(KYLENE);
	elseif (sequence == SEQ_085) then
		quest:SetENpc(FAUSTIGEANT, QFLAG_TALK);
		quest:SetENpc(NOGELOIX);
	elseif (sequence == SEQ_090) then
		-- The sickroom ward (PA 209/5 at the Phrontistery). The door
		-- (armed here) leads to the Warburton echo.
		quest:SetENpc(WARD_DOOR, QFLAG_PUSH, false, true);
		quest:SetENpc(SLOW_WITTED_MINER);
		quest:SetENpc(MUSCULAR_MINER);
		quest:SetENpc(QUALMISH_MINER);
		quest:SetENpc(WORRISOME_ASSISTANT);
		quest:SetENpc(WELL_WASHED_LEECH);
	elseif (sequence == SEQ_095) then
		quest:SetENpc(FAUSTIGEANT, QFLAG_TALK);
		quest:SetENpc(WARD_DOOR);
		quest:SetENpc(SLOW_WITTED_MINER);
		quest:SetENpc(MUSCULAR_MINER);
		quest:SetENpc(QUALMISH_MINER);
		quest:SetENpc(WORRISOME_ASSISTANT);
		quest:SetENpc(WELL_WASHED_LEECH);
	elseif (sequence == SEQ_100) then
		-- Waiting on the linkpearl read; the ward door doubles as the
		-- exit back to the Phrontistery public floor.
		quest:SetENpc(WARD_DOOR, QFLAG_PUSH, false, true);
		quest:SetENpc(FAUSTIGEANT);
	elseif (sequence == SEQ_105) then
		quest:SetENpc(MOMODI, QFLAG_REWARD);
		quest:SetENpc(WARD_DOOR, QFLAG_PUSH, false, true);
    end
end

-- The miner-instance crowd shared by SEQ_050/057 (pmeteor header's
-- name/class/event table).
function seq050_common_cast(quest, skipMiners)
	quest:SetENpc(LINETTE);
	quest:SetENpc(CORGUEVAIS_SCENE);
	quest:SetENpc(NITTMA_GUTTMA);
	quest:SetENpc(TYAGO_MOUI);
	quest:SetENpc(SHILGEN);
	quest:SetENpc(NORTMOEN);
	quest:SetENpc(MAUDLIN_MINER);
	quest:SetENpc(MOCKING_MINER);
	quest:SetENpc(MONITORING_MINER);
	quest:SetENpc(DISPLEASED_DANCER);
	quest:SetENpc(MUSCULAR_MINER);
	quest:SetENpc(CLOSE_FISTED_WOMAN);
	quest:SetENpc(ASTONISHED_ADVENTURER);
	if (not skipMiners) then
		quest:SetENpc(MANIC_MINER);
		quest:SetENpc(MADDENED_MINER);
	end
end

function onTalk(player, quest, npc)
    local sequence = quest:getSequence();
    local classId = npc:GetActorClassId();
	local data = quest:GetData();

    if (sequence == SEQ_000) then
        if (seq000_onTalk(player, quest, npc, classId) == true) then
            return; -- warped — nothing may follow (man0g1 SEQ_000 rule)
        end
    elseif (sequence == SEQ_005) then
        if (classId == MOMODI) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent010_2");
			player:EndEvent();
		end
	elseif (sequence == SEQ_010) then
		if (classId == MOMODI) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent015");
			player:EndEvent();
			quest:StartSequence(SEQ_012);
		end
	elseif (sequence == SEQ_012) then
		if (classId == MOMODI) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent017");
			-- "You obtain a Coliseum pass." — the key-item toast shape
			-- man0l1 uses for Baderon's Recommendation (25117).
			player:SendGameMessage(GetWorldMaster(), 25117, 0x20, ITEM_COLISEUM_PASS);
			player:EndEvent();
			quest:StartSequence(SEQ_015);
		end
	elseif (sequence == SEQ_015) then
		if (seq015_onTalk(player, quest, npc, classId) == true) then
			return;
		end
	elseif (sequence == SEQ_045) then
		if (classId == LINETTE) then
			-- man0u150: Linette's reception + the brawl-echo intro
			-- cutscene; ends fadeInAfterWarp → the PA warp below
			-- resolves the veil (retail: Now Loading → instance #1).
			callClientFunction(player, "delegateEvent", player, quest, "processEvent050");
			quest:StartSequence(SEQ_050);
			player:EndEvent();
			GetWorldManager():DoZoneChange(player, 209, "PrivateAreaMasterPast", 5, 15, -97.0, 195.6, 308.0, -0.5);
			return;
		end
	elseif (sequence == SEQ_050) then
		seq050_onTalk(player, quest, npc, classId);
	elseif (sequence == SEQ_057) then
		seq057_onTalk(player, quest, npc, classId);
	elseif (sequence == SEQ_058) then
		if (classId == CORGUEVAIS_SCENE) then
			-- man0u160: Corguevais's thanks + Linette's Camp Black
			-- Brush assignment + F'lhaminn volunteering, one cutscene
			-- (retail CS9); veil resolved by the exit warp to public.
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060");
			quest:StartSequence(SEQ_060);
			player:EndEvent();
			GetWorldManager():DoZoneChange(player, 209, nil, 0, 15, -95.0, 195.6, 305.0, 1.6);
			return;
		else
			seq058_onTalk(player, quest, npc, classId);
		end
	elseif (sequence == SEQ_060) then
		if (classId == FLHAMINN) then
			-- Gate-side wait talk (texts 144-146 — no client event
			-- exists for these; server-side says, man0l1
			-- seq007_endSequence precedent).
			player:SendGameMessage(quest, 144, 0x20);
			player:SendGameMessage(quest, 145, 0x20);
			player:SendGameMessage(quest, 146, 0x20);
		end
		player:EndEvent();
	elseif (sequence == SEQ_065) then
		if (seq065_onTalk(player, quest, npc, classId) == true) then
			return;
		end
	elseif (sequence == SEQ_070) then
		if (classId == MOMODI) then
			-- Momodi's F'lhaminn gossip (texts 389/390).
			callClientFunction(player, "delegateEvent", player, quest, "processEvent200_2");
			player:EndEvent();
			quest:StartSequence(SEQ_075);
		end
	elseif (sequence == SEQ_075) then
		if (classId == FLHAMINN) then
			-- man0u200: the escort payment at the Concern stage — an
			-- in-place cutscene (fadeInCutSceneDefault; no warp
			-- needed). "You obtain 3,000 gil." is on camera.
			callClientFunction(player, "delegateEvent", player, quest, "processEvent200");
			player:AddGil(3000);
			player:EndEvent();
			quest:NewNpcLsMsg(1);
		end
	elseif (sequence == SEQ_080) then
		if (classId == NOGELOIX) then
			-- man0u205: reception (in-place cutscene).
			callClientFunction(player, "delegateEvent", player, quest, "processEvent205");
			player:EndEvent();
			quest:StartSequence(SEQ_085);
		end
	elseif (sequence == SEQ_085) then
		if (classId == FAUSTIGEANT) then
			-- man0u210: the Checheroya cave-in exchange + the sickroom
			-- survey assignment (in-place cutscene). The ward door
			-- arms at SEQ_090.
			callClientFunction(player, "delegateEvent", player, quest, "processEvent210");
			player:EndEvent();
			quest:StartSequence(SEQ_090);
		elseif (classId == NOGELOIX) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent210_2");
			player:EndEvent();
		end
	elseif (sequence == SEQ_090) then
		seq090_onTalk(player, quest, npc, classId);
	elseif (sequence == SEQ_095) then
		if (classId == FAUSTIGEANT) then
			-- man0u230: the no-pay report ("It doesn't even warrant a
			-- wage", texts 218/219) → Momodi's glow.
			callClientFunction(player, "delegateEvent", player, quest, "processEvent230");
			player:EndEvent();
			quest:NewNpcLsMsg(1);
			quest:StartSequence(SEQ_100);
		else
			seq090_onTalk(player, quest, npc, classId);
		end
	elseif (sequence == SEQ_105) then
		if (classId == MOMODI) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventComplete");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 300, 1, 1, 2);
			-- Completion: 200 EXP + 6,000 gil — client
			-- quest_new_reward.csv row 110010; the 6,000-gil toast is
			-- on camera at the retail turn-in.
			player:AddExp(200, player.charaWork.parameterSave.state_mainSkill[0], 0);
			player:AddGil(6000);
			player:EndEvent();
			player:CompleteQuest(quest);
			return;
		end
    end

	-- Blanket close: registered-but-unhandled talks (bare SetENpc
	-- flavor NPCs) must still get their EndEvent or the client is
	-- event-locked; a second EndEvent after a branch's own close is
	-- proven harmless (the live man0g1 seq000 shape).
	player:EndEvent();
	quest:UpdateENPCs();
end

function seq000_onTalk(player, quest, npc, classId)
    if (classId == MOMODI) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
        player:EndEvent();
        quest:StartSequence(SEQ_005);
		quest:UpdateENPCs();
        GetWorldManager():DoZoneChange(player, 175, nil, 0, 15, player.positionX, player.positionY, player.positionZ, player.rotation);
        return true;
    elseif (classId == UNDAUNTED_ADVENTURER) then
        callClientFunction (player, "delegateEvent", player, quest, "processEvent000_2");
    elseif (classId == GREEDY_MERCHANT) then
        callClientFunction (player, "delegateEvent", player, quest, "processEvent000_3");
    elseif (classId == SPRY_SALESMAN) then
        callClientFunction (player, "delegateEvent", player, quest, "processEvent000_4");
    elseif (classId == LIONHEARTED_ADVENTURER) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_5");
    elseif (classId == UPBEAT_ADVENTURER) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_6");
    elseif (classId == SEEMINGLY_CALM_ADVENTURER) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_7");
    elseif (classId == OVERCOMPETITIVE_ADVENTURER) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_8");
    elseif (classId == OTOPA_POTTOPA) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_9");
    elseif (classId == THANCRED) then
        callClientFunction(player, "delegateEvent", player, quest, "processEvent000_10");
    end

    player:EndEvent();
end

function seq015_onTalk(player, quest, npc, classId)
	local data = quest:GetData();
	local subseqGSM = data:GetCounter(CNTR_SEQ15_GSM);
	local subseqGLD = data:GetCounter(CNTR_SEQ15_GLD);

	if (classId == MOMODI) then
		if (subseqGSM >= 1 and subseqGLD < 5) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent017_3");
		elseif (subseqGLD >= 5 and subseqGSM == 0) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent017_4");
		else
			callClientFunction(player, "delegateEvent", player, quest, "processEvent017_2");
		end
	-- GSM leg
	elseif (classId == ELECOTTE) then
		if (subseqGSM == 0) then
			-- man0u120: the Niellefresne appraisal echo — an in-place
			-- cutscene (fadeInCutSceneDefault). "the sum of two
			-- thousand gil" (text 44) + the retail 2,000-gil toast.
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020");
			data:IncCounter(CNTR_SEQ15_GSM);
			player:AddGil(2000);
			if (data:GetCounter(CNTR_SEQ15_GLD) >= 5) then
				seq015_endSequence(player, quest);
			end
		else
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020_2");
		end
	-- GLD leg
	elseif (classId == YOYOBINA) then
		if (subseqGLD == 0) then
			-- man0u130: the Yoyobina briefing + Greinfarr recruitment
			-- echo (retail CS4); ends fadeInAfterWarp → resolved by
			-- the lobby PA warp ("You have entered an instance").
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030");
			data:IncCounter(CNTR_SEQ15_GLD);
			player:EndEvent();
			quest:UpdateENPCs();
			GetWorldManager():DoZoneChange(player, 209, "PrivateAreaMasterPast", 5, 15, -185.0, 195.0, 185.0, -2.3);
			return true;
		else
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030_3");
		end
	elseif (classId == LULUTSU) then
		if (subseqGLD == 1) then
			-- The Coliseum Pass check ("another bloody Coliseum Pass,
			-- is it? ... Now go and wait within." — texts 56-58). Arms
			-- the arena-gate trigger.
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030_2");
			data:IncCounter(CNTR_SEQ15_GLD);
		elseif (subseqGLD >= 2) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030_2");
		else
			-- Pre-briefing (only reachable via a relog inside the PA):
			-- generic parade-gossip line rather than a pass check the
			-- story hasn't set up yet.
			callClientFunction(player, "delegateEvent", player, quest, "processEvent1000_1");
		end
	elseif (classId == GREINFARR) then
		if (subseqGLD == 4) then
			-- man0u140: the Greinfarr/Niellefresne "gathering" echo
			-- (retail CS6; design-doc checkpoint 2 — retail auto-plays
			-- it on arrival, we fire it from his talk). Ends
			-- fadeInAfterWarp → resolved by the warp back to the
			-- Quicksand, where Momodi's linkpearl glow fires.
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040");
			data:IncCounter(CNTR_SEQ15_GLD);
			player:EndEvent();
			if (data:GetCounter(CNTR_SEQ15_GSM) >= 1) then
				seq015_endSequence(player, quest);
			end
			quest:UpdateENPCs();
			GetWorldManager():DoZoneChange(player, 175, nil, 0, 15, -75.242, 195.009, 74.572, -0.046);
			return true;
		end
	-- Ambient Coliseum crowd (classic-wiki processEvent table).
	elseif (classId == PAPAWA) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent030_4");
	elseif (classId == ABYLGO_HAMYLGO) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent030_8");
	elseif (classId == FRUHYBOLG) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent030_6");
	elseif (classId == SWERDAHRM) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent030_7");
	elseif (classId == NIELLEFRESNE_GLD) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent030_9");
	elseif (classId == HNAUFRID) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent032_2");
	elseif (classId == PAMISOLAUX) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent032_3");
	end

	player:EndEvent();
end

-- Both SEQ_015 legs done → queue Momodi's linkpearl call (pack 1: the
-- Amajina recruiting pitch). The LS read advances to SEQ_045.
function seq015_endSequence(player, quest)
	quest:NewNpcLsMsg(1);
end

function seq050_onTalk(player, quest, npc, classId)
	local data = quest:GetData();

	if (classId == FLHAMINN_SCENE) then
		local step = data:GetCounter(CNTR_EMOTE_TEACH);
		if (step < 6) then
			-- Teach (or re-teach) the current emote: 051_1..051_6 play
			-- her demo via the client chara scheduler and say the
			-- teach line (344-349).
			callClientFunction(player, "delegateEvent", player, quest, "processEvent051_" .. (step + 1));
		else
			-- All six taught — the go-signal (text 350).
			callClientFunction(player, "delegateEvent", player, quest, "processEvent051_8");
		end
	elseif (classId == LINETTE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_2");
	elseif (classId == MAUDLIN_MINER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_3");
	elseif (classId == MOCKING_MINER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_4");
	elseif (classId == MONITORING_MINER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_5");
	elseif (classId == DISPLEASED_DANCER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_6");
	elseif (classId == MUSCULAR_MINER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_7");
	elseif (classId == CLOSE_FISTED_WOMAN) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_8");
	elseif (classId == ASTONISHED_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_9");
	elseif (classId == NITTMA_GUTTMA) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_10");
	elseif (classId == CORGUEVAIS_SCENE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_11");
	elseif (classId == TYAGO_MOUI) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_12");
	elseif (classId == MANIC_MINER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_13");
	elseif (classId == MADDENED_MINER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_14");
	end

	player:EndEvent();
end

function seq057_onTalk(player, quest, npc, classId)
	if (classId == FLHAMINN_SCENE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent051_8");
	elseif (classId == MANIC_MINER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_13");
	elseif (classId == MADDENED_MINER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_14");
	else
		seq050_onTalk(player, quest, npc, classId);
		return;
	end
	player:EndEvent();
end

function seq058_onTalk(player, quest, npc, classId)
	-- Instance #2 crowd (texts 114-118, 136 → events 060_2..060_7).
	if (classId == MAUDLIN_MINER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent060_2");
	elseif (classId == MONITORING_MINER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent060_3");
	elseif (classId == MUSCULAR_MINER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent060_4");
	elseif (classId == NITTMA_GUTTMA) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent060_5");
	elseif (classId == POPOKKULI) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent060_6");
	elseif (classId == SESERUKKA) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent060_7");
	elseif (classId == FLHAMINN_SCENE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent1000_3");
	elseif (classId == LINETTE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_2");
	end
	player:EndEvent();
end

function seq065_onTalk(player, quest, npc, classId)
	local data = quest:GetData();

	if (classId == ASCILIA_CAMP and not data:GetFlag(FLAG_CAMP_ECHO_SEEN)) then
		-- man0u180: the Ascilia/Corguevais/Thancred echo (retail
		-- CS11+CS12). Ends fadeInAfterWarp — the same-zone public
		-- reload below resolves the veil (the man0l1 seq000 same-zone
		-- warp recipe; design-doc checkpoint 3).
		callClientFunction(player, "delegateEvent", player, quest, "processEvent080");
		data:SetFlag(FLAG_CAMP_ECHO_SEEN);
		-- F'lhaminn's post-echo pivot to the camp leader (text 176).
		player:SendGameMessage(quest, 176, 0x20);
		player:EndEvent();
		quest:UpdateENPCs();
		GetWorldManager():DoZoneChange(player, 170, nil, 0, 15, player.positionX, player.positionY, player.positionZ, player.rotation);
		return true;
	elseif (classId == HELPLESS_HYUR and data:GetFlag(FLAG_CAMP_ECHO_SEEN)) then
		-- The camp leader rebuffs F'lhaminn (texts 177-183 — no client
		-- event exists for the exchange; server-side says), then the
		-- pair returns to Ul'dah (retail: Now Loading straight after
		-- "Let us return").
		player:SendGameMessage(quest, 177, 0x20);
		player:SendGameMessage(quest, 178, 0x20);
		player:SendGameMessage(quest, 179, 0x20);
		player:SendGameMessage(quest, 180, 0x20);
		player:SendGameMessage(quest, 181, 0x20);
		player:SendGameMessage(quest, 182, 0x20);
		player:SendGameMessage(quest, 183, 0x20);
		quest:StartSequence(SEQ_070);
		player:EndEvent();
		quest:UpdateENPCs();
		GetWorldManager():DoZoneChange(player, 175, nil, 0, 15, -75.242, 195.009, 74.572, -0.046);
		return true;
	elseif (classId == UNTIDY_OUTLANDER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent080_2");
	elseif (classId == MALNOURISHED_MIDLANDER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent080_3");
	elseif (classId == CORGUEVAIS) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent080_10");
	elseif (classId == THANCRED_CAMP) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent080_11");
	elseif (classId == HELPLESS_HYUR) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent080_9");
	end

	player:EndEvent();
end

function seq090_onTalk(player, quest, npc, classId)
	-- The sickroom ward survey (texts 386-388, 198-200).
	if (classId == SLOW_WITTED_MINER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent210_6");
	elseif (classId == MUSCULAR_MINER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent210_7");
	elseif (classId == QUALMISH_MINER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent210_8");
	elseif (classId == WORRISOME_ASSISTANT) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent210_3");
	elseif (classId == WELL_WASHED_LEECH) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent210_4");
	end
	player:EndEvent();
end

function onPush(player, quest, npc)
    local sequence = quest:getSequence();
    local classId = npc:GetActorClassId();
	local data = quest:GetData();

	if (sequence == SEQ_015) then
		if (classId == ARENA_TRIGGER and data:GetCounter(CNTR_SEQ15_GLD) == 2) then
			-- Retail: "Duty calls. Do you wish to proceed with 'Court
			-- in the Sands'?" — the same duty-join dialog as the
			-- sibling escorts.
			local result = callClientFunction(player, "delegateEvent", player, quest, "contentsJoinAskInBasaClass");
			if (result == 1) then
				startMan0u1Coliseum(player, quest);
				return;
			end
			player:EndEvent();
		end
	elseif (sequence == SEQ_060) then
		if (classId == GATE_OF_NALD_TRIGGER) then
			local result = callClientFunction(player, "delegateEvent", player, quest, "contentsJoinAskInBasaClass");
			if (result == 1) then
				startMan0u1Escort(player, quest);
				return;
			end
			player:EndEvent();
		end
	elseif (sequence == SEQ_090) then
		if (classId == WARD_DOOR) then
			-- man0u220: the Warburton-death echo (retail CS17). Ends
			-- fadeInAfterWarp — resolved by the same-PA reload below
			-- (retail stays instanced through the echo; design-doc
			-- checkpoint 1). Momodi's glow fires at SEQ_095's report.
			callClientFunction(player, "delegateEvent", player, quest, "processEvent220");
			quest:StartSequence(SEQ_095);
			player:EndEvent();
			quest:UpdateENPCs();
			GetWorldManager():DoZoneChange(player, 209, "PrivateAreaMasterPast", 5, 15, -218.0, 229.5, 306.0, 0.0);
			return;
		end
	elseif (sequence == SEQ_100 or sequence == SEQ_105) then
		if (classId == WARD_DOOR) then
			-- Post-report the door is the exit back to the public
			-- Phrontistery floor.
			player:EndEvent();
			quest:UpdateENPCs();
			GetWorldManager():DoZoneChange(player, 209, nil, 0, 15, -211.7, 229.6, 279.0, -2.7);
			return;
		end
	end
	quest:UpdateENPCs();
end

function onEmote(player, quest, npc, eventName)
	local data = quest:GetData();
	local sequence = quest:getSequence();
	local classId = npc:GetActorClassId();

	-- Which of the six taught emotes did the player perform? (The
	-- emoteDefaultN conditions are seeded in teach order on F'lhaminn
	-- and both miners — seed/095.)
	local performed = nil;
	for i = 1, #EMOTE_STEPS do
		if (eventName == EMOTE_STEPS[i].eventName) then
			performed = i;
			break;
		end
	end
	if (performed ~= nil) then
		player:DoEmote(npc.Id, EMOTE_STEPS[performed].doEmoteId, EMOTE_STEPS[performed].motionMsg);
		wait(2.5);
	end

	if (sequence == SEQ_050 and classId == FLHAMINN_SCENE and performed ~= nil) then
		local step = data:GetCounter(CNTR_EMOTE_TEACH);
		if (step < 6) then
			if (performed == step + 1) then
				data:IncCounter(CNTR_EMOTE_TEACH);
				if (step + 1 < 6) then
					-- Next lesson (she acks the success inside the
					-- next 051_x teach line — texts 345-349 all open
					-- with the "Yes, just so" family).
					callClientFunction(player, "delegateEvent", player, quest, "processEvent051_" .. (step + 2));
				else
					-- Soothe done — the go-signal (text 350), then the
					-- calming phase.
					callClientFunction(player, "delegateEvent", player, quest, "processEvent051_8");
					player:EndEvent();
					quest:StartSequence(SEQ_057);
					return;
				end
			else
				-- Wrong emote: "What? You've forgotten already!?" +
				-- she replays the demo for the step the player is
				-- stuck on (the only 4-arg delegate in the client
				-- script — selector 51..56).
				callClientFunction(player, "delegateEvent", player, quest, "processEvent051_7", 50 + step + 1);
			end
		end
	elseif (sequence == SEQ_057 and performed ~= nil) then
		if (classId == MADDENED_MINER and not data:GetFlag(FLAG_CALM_MADDENED)) then
			if (performed == MADDENED_STEP) then
				data:SetFlag(FLAG_CALM_MADDENED);
			else
				-- Rebuff (056_1..3, texts 353/355/357), rotating on
				-- the wrong-emote index so the barks vary.
				callClientFunction(player, "delegateEvent", player, quest, "processEvent056_" .. ((performed % 3) + 1));
			end
		elseif (classId == MANIC_MINER and data:GetCounter(CNTR_MANIC_CALM) < #MANIC_SEQUENCE) then
			local manicStep = data:GetCounter(CNTR_MANIC_CALM);
			if (performed == MANIC_SEQUENCE[manicStep + 1]) then
				data:IncCounter(CNTR_MANIC_CALM);
			else
				-- Rebuff (055_1..3, texts 352/354/356).
				callClientFunction(player, "delegateEvent", player, quest, "processEvent055_" .. ((performed % 3) + 1));
			end
		end
		-- Both becalmed → the Popokkuli & Seserukka ending (texts
		-- 109, 107-113 — no client event exists for the scene;
		-- server-side says, the man0l1 seq007_endSequence recipe),
		-- then the reload into instance #2.
		if (data:GetFlag(FLAG_CALM_MADDENED) and data:GetCounter(CNTR_MANIC_CALM) >= #MANIC_SEQUENCE) then
			player:SendGameMessage(quest, 109, 0x20);
			player:SendGameMessage(quest, 107, 0x20);
			player:SendGameMessage(quest, 108, 0x20);
			player:SendGameMessage(quest, 110, 0x20);
			player:SendGameMessage(quest, 111, 0x20);
			player:SendGameMessage(quest, 112, 0x20);
			player:SendGameMessage(quest, 113, 0x20);
			quest:StartSequence(SEQ_058);
			player:EndEvent();
			quest:UpdateENPCs();
			GetWorldManager():DoZoneChange(player, 209, "PrivateAreaMasterPast", 5, 15, -97.0, 195.6, 308.0, -0.5);
			return;
		end
	end

	player:EndEvent();
	quest:UpdateENPCs();
end

function onNotice(player, quest, target)
	local sequence = quest:getSequence();

	if (sequence == SEQ_000) then
		-- The "what an Instance is" explainer (worldMaster says
		-- 329/330 client-side) — pmeteor's implemented behavior,
		-- sequence-gated so stray kicks can't replay it. Driven with
		-- the RAW non-parking RunEventFunction: the client body is two
		-- bare worldMaster.say calls with NO talk-turn/cutscene/_wait,
		-- i.e. the processEventTu_001 no-EventUpdate shape whose
		-- callClientFunction park-forever event-lock is documented at
		-- man0l1's onNotice — the raw form lets the EndEvent below
		-- close the notice. (Currently no kick routes to 110010, so
		-- this arm is dormant; kept park-safe for when the instance-
		-- explainer beat gets wired.)
		player:RunEventFunction("delegateEvent", player, quest, "processEvent000_1");
		player:EndEvent();
	else
		-- Safety net: any stray noticeEvent kick (e.g. a content
		-- director's) landing after the sequence advanced only needs
		-- the EndEvent to close — leaving it open event-locks the
		-- client (man0l1/man0g1 onNotice rule).
		player:EndEvent();
	end
	quest:UpdateENPCs();
end

function onNpcLS(player, quest, from, msgStep)
	local sequence = quest:getSequence();
	local msgPack;

	if (from == 1) then
		if (sequence == SEQ_015 or sequence == SEQ_045) then
			msgPack = 1;
		elseif (sequence == SEQ_075 or sequence == SEQ_080) then
			msgPack = 2;
		elseif (sequence == SEQ_100 or sequence == SEQ_105) then
			msgPack = 3;
		end
		if (msgPack == nil) then
			player:EndEvent();
			return;
		end

		player:SendGameMessageLocalizedDisplayName(quest, NPCLS_MSGS[msgPack][msgStep], MESSAGE_TYPE_NPC_LINKSHELL, MOMODI_DISPLAY_ID);
		if (msgStep >= #NPCLS_MSGS[msgPack]) then
			quest:EndOfNpcLsMsgs();
		else
			quest:ReadNpcLsMsg();
		end

		if (sequence == SEQ_015) then
			quest:StartSequenceForNpcLs(SEQ_045);
		elseif (sequence == SEQ_075) then
			quest:StartSequenceForNpcLs(SEQ_080);
		elseif (sequence == SEQ_100) then
			quest:StartSequenceForNpcLs(SEQ_105);
		end
	end

	player:EndEvent();
end

-- ===== The Coliseum first-round match (Garlemald-Server #53) =====
-- Retail shape (both full playthroughs): instance entry → "Duty
-- calls" confirm → Now Loading → "You are now bound by duty." → an
-- UNWINNABLE 1v1 vs the Tourney Gladiator (2280157) — "Otto/Siddeon
-- is defeated." — → "A splendid match! I expected no less. / Don't
-- let losing get to you." Loss IS the completion condition (Mirke
-- footnote: "There is no way to win."; the beta uploader: the
-- gladiator is immortal, you lose by KO or timeout).
--
-- Content shape mirrors the wire-proven sibling escorts: rescue latch
-- → CreateContentArea → director kick (Rust defers the 0x012F TX past
-- the warp ack) → EndEvent BEFORE the warp → DoZoneChangeContent
-- (spawnType 16).
function startMan0u1Coliseum(player, quest)
	local data = quest:GetData();
	data:SetFlag(FLAG_ARENA_HANDOFF);
	data:IncCounter(CNTR_SEQ15_GLD); -- 2 → 3 (fight handed off)
	-- Consume the latch NOW: UpdateENPCs re-runs onStateChange, whose
	-- GLD==3 arm sees the flag SET and clears it (the StartSequence-
	-- fired consume of the sibling escorts, which don't apply here
	-- because the arena doesn't advance the sequence). Any LATER
	-- arrival at GLD==3 — the relog re-arm after the content died with
	-- the session — sees it CLEAR and rolls back to 2 so the arena
	-- trigger re-arms for a retry.
	quest:UpdateENPCs();

	local contentArea = player.CurrentArea:CreateContentArea(player, "/Area/PrivateArea/Content/PrivateAreaMasterSimpleContent", "Man0u101", "SimpleContentMan0u101", "Quest/QuestDirectorMan0u101", 209);
	if (contentArea == nil) then
		return;
	end
	local director = contentArea:GetContentDirector();
	player:AddDirector(director);
	director:StartDirector(false);
	player:KickEvent(director, "noticeEvent", true);
	player:SetLoginDirector(director);

	-- Close the push event BEFORE the warp (0x0131 ahead of the
	-- 0x00E2 reload latch — retail invariant; a mid-reload EndEvent is
	-- the desktopWidgetMode-16 menu lock). No pre-warp cutscene here:
	-- retail cuts straight from the confirm to the Now-Loading into
	-- the arena.
	player:EndEvent();

	-- The bloodsands. Anchored at the GC-quest coliseum marker
	-- (11092101 → -205.4, 150.9; floor y from the lobby rows) — the
	-- only in-data arena anchor; flagged for in-client tuning
	-- (seed/096). Offset a few units from the gladiator's spawn point
	-- (the sibling escorts' player-vs-duty-NPC spacing) so the player
	-- doesn't materialize inside the opponent's model.
	GetWorldManager():DoZoneChangeContent(player, contentArea, -209.9, 192.0, 146.4, 0.8, 16);
end

-- ===== The F'lhaminn escort (Garlemald-Server #53) =====
-- Gate of Nald → Camp Black Brush, zone 170 (both endpoints on the
-- field map per the quest_marker sheet: gate -34.5/-68.9 → camp
-- 34.8/-480.5). 30-minute duty, chinchilla (2204010) ambushes that
-- one-shot-die at opener levels (retail: 60-80 dmg Light Slash kills,
-- 511-650 EXP each). The startMan0l1Content/startMan0g1Content
-- 10-step shape, wire-proven on both siblings.
function startMan0u1Escort(player, quest)
	local contentArea = player.CurrentArea:CreateContentArea(player, "/Area/PrivateArea/Content/PrivateAreaMasterSimpleContent", "Man0u102", "SimpleContentMan0u102", "Quest/QuestDirectorMan0u102", 170);
	if (contentArea == nil) then
		return;
	end
	local director = contentArea:GetContentDirector();
	player:AddDirector(director);
	director:StartDirector(false);
	player:KickEvent(director, "noticeEvent", true);
	player:SetLoginDirector(director);

	-- man0u170: the gate-side duty-start cutscene IN PLACE (arms the
	-- after-warp veil; the director's questBaseRewardSeting dismisses
	-- it post-warp).
	callClientFunction(player, "delegateEvent", player, quest, "processEvent070");

	player:EndEvent();

	-- Duty warp to the gate-side start of the escort route
	-- (spawnType 16 → wipe + 0x00E2(0x10) + 34108 "You have entered
	-- an instance." + zone-in bundle). NOTHING may follow this call.
	GetWorldManager():DoZoneChangeContent(player, contentArea, -34.5, 183.0, -68.9, -3.0, 16);
end

function getJournalInformation(player, quest)
	local sequence = quest:getSequence();
	local data = quest:GetData();
	-- Journal qtdata args (pmeteor stub shape): the Velodyna Cosmos
	-- shows while the flower is still in hand (sold at the GSM leg),
	-- the Coliseum Pass from its grant until the fight is done.
	local velodyna = 0;
	local pass = 0;
	if (sequence <= SEQ_015 and data:GetCounter(CNTR_SEQ15_GSM) == 0) then
		velodyna = ITEM_VELODYNA_COSMOS;
	end
	if (sequence >= SEQ_012 and sequence <= SEQ_015 and data:GetCounter(CNTR_SEQ15_GLD) < 4) then
		pass = ITEM_COLISEUM_PASS;
	end
	return 0, velodyna, pass;
end

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
	local data = quest:GetData();
    local possibleMarkers = {};

    if (sequence == SEQ_000 or sequence == SEQ_010 or sequence == SEQ_012 or sequence == SEQ_070 or sequence == SEQ_105) then
        table.insert(possibleMarkers, MRKR_MOMODI);
    elseif (sequence == SEQ_005) then
        table.insert(possibleMarkers, MRKR_CAMP_BLACK_BRUSH);
    elseif (sequence == SEQ_015) then
		if (data:GetCounter(CNTR_SEQ15_GSM) == 0) then
			table.insert(possibleMarkers, MRKR_ESHTAIME);
		end
		if (data:GetCounter(CNTR_SEQ15_GLD) < 5) then
			table.insert(possibleMarkers, MRKR_COLISEUM);
		end
	elseif (sequence == SEQ_045) then
		table.insert(possibleMarkers, MRKR_LINETTE);
	elseif (sequence == SEQ_050) then
		table.insert(possibleMarkers, MRKR_FLHAMINN_TEACH);
	elseif (sequence == SEQ_057) then
		if (data:GetCounter(CNTR_MANIC_CALM) < #MANIC_SEQUENCE) then
			table.insert(possibleMarkers, MRKR_MANIC_MINER);
		end
		if (not data:GetFlag(FLAG_CALM_MADDENED)) then
			table.insert(possibleMarkers, MRKR_MADDENED_MINER);
		end
	elseif (sequence == SEQ_060) then
		table.insert(possibleMarkers, MRKR_GATE_OF_NALD);
	elseif (sequence == SEQ_065) then
		table.insert(possibleMarkers, MRKR_CAMP_LEADER);
	elseif (sequence == SEQ_075) then
		table.insert(possibleMarkers, MRKR_CONCERN_STAGE);
	elseif (sequence == SEQ_080 or sequence == SEQ_085) then
		table.insert(possibleMarkers, MRKR_PHRONTISTERY);
	elseif (sequence == SEQ_090 or sequence == SEQ_095) then
		table.insert(possibleMarkers, MRKR_WARD_DOOR);
    end

    return unpack(possibleMarkers);
end
