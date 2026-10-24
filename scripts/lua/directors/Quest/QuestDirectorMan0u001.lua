require ("global")
require ("tutorial")
require ("modifiers")
require ("quests/man/man0u0")

--processTtrBtl001: Active Mode Tutorial
--processTtrBtl002: Targetting Tutorial (After active mode done)
--processTtrBtl003: Auto Attack Done
--processTtrBtl004: Tutorial Complete

-- Ul'dah SEQ_005 Sapphire Avenue battle director — port of the
-- QuestDirectorMan0g001 rewrite (Garlemald-Server #28 phases) to Man0u0
-- (#26 follow-up, sibling of the #25 Limsa port). The upstream import
-- drove the fight with fixed wait(4)/wait(6) chains — the exact shape
-- that stalled Gridania after the first cinematic — and never gated on
-- the actual kill. Spawning + roster live in
-- content/SimpleContent30079.lua's onCreate; this director only drives
-- the tutorial beats.

function init()
	return "/Director/Quest/QuestDirectorMan0u001";
end

function onEventStarted(player, actor, triggerName)
	man0u0Quest = player:GetQuest("Man0u0");
	startTutorialMode(player);                      -- SendDataPacket(9): idempotent

	if player:IsDiscipleOfWar() then
		-- Active-mode cutscene + "draw your weapon"; returns when the
		-- player dismisses the ACTIVEMODE popup (widget 6 yields).
		callClientFunction(player, "delegateEvent", player, man0u0Quest, "processTtrBtl001");
		player:EndEvent();
		waitForSignal("playerActive");              -- F press (0x0134)
		-- Kept for pmeteor parity ("If this isn't here, the scripts bugs
		-- out" — see the Man0g001 risk note).
		wait(1);
		kickEventContinue(player, actor, "noticeEvent", "noticeEvent");
		-- Targeting tutorial. Only returns after the player targets the
		-- live goobbue — the content script's engagement latch keeps it
		-- alive until here.
		callClientFunction(player, "delegateEvent", player, man0u0Quest, "processTtrBtl002", nil, nil, nil);
		player:EndEvent();

		-- ==== THE FIGHT ====  (free-running: combat AI + skills)
		-- Milestone-gated tooltip chain; signal sources are Rust-side
		-- (`fire_content_signal`): "playerAttack" per player swing,
		-- "tpOver1000" per accrual at/above 1000, "weaponskillUsed" on a
		-- player weaponskill resolution, "battleComplete" when the
		-- content roster has zero live hostiles.
		waitForSignal("playerAttack");
		closeTutorialWidget(player);
		showTutorialSuccessWidget(player, 9055);    -- battle-action success
		openTutorialWidget(player, CONTROLLER_KEYBOARD, TUTORIAL_TP);
		waitForSignal("tpOver1000");
		player:SetMod(modifiersGlobal.MinimumTpLock, 1000);  -- can't dribble below the WS cost mid-lesson
		closeTutorialWidget(player);
		openTutorialWidget(player, CONTROLLER_KEYBOARD, TUTORIAL_WEAPONSKILLS);
		waitForSignal("weaponskillUsed");
		player:SetMod(modifiersGlobal.MinimumTpLock, 0);
		closeTutorialWidget(player);
		showTutorialSuccessWidget(player, 9065);    -- weaponskill success

		waitForSignal("battleComplete");            -- the goobbue is down
		-- Render-settle beat (see Man0g001): without it the defeat dialog
		-- lands in the same drain as the death packets.
		wait(3);
		closeTutorialWidget(player);
	elseif player:IsDiscipleOfMagic() then
		-- THM starters take the magic variant, same as the Gridania and
		-- Limsa ports.
		callClientFunction(player, "delegateEvent", player, man0u0Quest, "processTtrBtlMagic001");
		player:EndEvent();
		wait(1);
		kickEventContinue(player, actor, "noticeEvent", "noticeEvent");
		-- The kicked noticeEvent must be ended again: left open, the client
		-- stays in event mode, so F (active mode) and attacks are dead and
		-- the fight can never start. Same targeting lesson + EndEvent as the
		-- DoW branch; it returns once a wolf is targeted, with movement,
		-- hotbar and targeting unlocked.
		callClientFunction(player, "delegateEvent", player, man0u0Quest, "processTtrBtl002", nil, nil, nil);
		player:EndEvent();
		closeTutorialWidget(player);
		openTutorialWidget(player, CONTROLLER_KEYBOARD, TUTORIAL_DEFEATENEMY);
		waitForSignal("battleComplete");
		wait(3);                                    -- render-settle beat
		closeTutorialWidget(player);
		showTutorialSuccessWidget(player, 9050);
	end

	worldMaster = GetWorldMaster();
	player:SendDataPacket("attention", worldMaster, "", 51073, 3);
	wait(2);                                        -- decoration only, not load-bearing
	player:ChangeMusic(7);
	player:ChangeState(0);                          -- sheathe → State trio flushes

	-- Reopen the event context BEFORE delegating (Man0g001 pattern —
	-- a bare delegate ships with owner=0 and the client echo-drops it).
	kickEventContinue(player, actor, "noticeEvent", "noticeEvent");
	-- Fade-out + the arrival cutscene chain + journal notify; ends
	-- expecting the warp right after (upstream's processEvent020).
	callClientFunction(player, "delegateEvent", player, man0u0Quest, "processEvent020");
	man0u0Quest:StartSequence(10);
	player:EndEvent();
	player:GetZone():ContentFinished();
	GetWorldManager():DoZoneChange(player, 175, "PrivateAreaMasterPast", 3, 15, -22.81, 196, 87.82, 2.98);
end

function onUpdate(deltaTime, area)
end

function onTalkEvent(player, npc)
end

function onPushEvent(player, npc)
end

function onCommandEvent(player, command)
end

function onEventUpdate(player, npc)
end

function onCommand(player, command)
end

-- NOTE: this director intentionally defines NO `main` and no
-- `onCreateContentArea` — same rationale as QuestDirectorMan0g001 /
-- Man0l001: the content group's spawning + roster are owned by
-- SimpleContent30079.lua's `onCreate`, and the content-group wire trio
-- is emitted Rust-side from apply_do_zone_change_content.
