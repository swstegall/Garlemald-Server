require ("global")
require ("tutorial")
require ("modifiers")
require ("quests/man/man0g0")

--processTtrBtl001: Active Mode Tutorial
--processTtrBtl002: Targetting Tutorial (After active mode done)

function init()
	return "/Director/Quest/QuestDirectorMan0g001";
end

function onEventStarted(player, actor, triggerName)
	-- #28 Phase 4: pmeteor's wait(3)/wait(3) fakery is replaced by the
	-- all-wolves-dead kill gate (check_content_battle_complete fires
	-- "battleComplete" through the event bridge when the content roster
	-- has zero live hostiles). NO endTutorialMode here — the client
	-- contract reserves SendDataPacket(7)/cancelTutorialMode for
	-- Man0g1's processEvent110; processEvent020_1's own trailing mask
	-- (f,f,f,t,t,3) is the correct town state.
	man0g0Quest = player:GetQuest("Man0g0");
	startTutorialMode(player);                      -- SendDataPacket(9): idempotent, nets mask (f,f,f,t,t,3)

	if player:IsDiscipleOfWar() then
		-- MAN0G010 cutscene + "draw your weapon"; trailing mask (t,t,t,F-armed,t,4).
		-- Returns when the player dismisses the ACTIVEMODE popup (widget 6 yields).
		callClientFunction(player, "delegateEvent", player, man0g0Quest, "processTtrBtl001");
		player:EndEvent();
		waitForSignal("playerActive");              -- F press (0x0134)
		-- Risk note: the live wait(1)-never-resumes break sits between
		-- playerActive and this kick (report E §2.3, instrumented by
		-- S1.4). Kept for pmeteor parity ("If this isn't here, the
		-- scripts bugs out" — about pmeteor's own engine); contingency
		-- is dropping it (garlemald's signal→kick handoff is synchronous).
		wait(1);
		kickEventContinue(player, actor, "noticeEvent", "noticeEvent");
		-- Targeting tutorial. ONLY returns after the player targets a live
		-- class-2201407 wolf; on return movement+hotbar+targeting are
		-- unlocked client-side (mask f,f,f,f,f,3). Requires wolves
		-- alive until here (the content script's engagement latch
		-- guarantees no combat yet).
		callClientFunction(player, "delegateEvent", player, man0g0Quest, "processTtrBtl002", nil, nil, nil);
		player:EndEvent();

		-- ==== THE FIGHT ====  (free-running: S2 AI + S3 skills)
		-- Milestone-gated tooltip chain (MeteorReborn QuestDirector
		-- Man0g001.lua DoW branch, Decimus's rescript — the canonical
		-- retail sequence). Signal sources are Rust-side
		-- (`fire_content_signal`): "playerAttack" per player swing,
		-- "tpOver1000" per accrual at/above 1000, "weaponskillUsed" on a
		-- player weaponskill resolution. The old shape skipped straight
		-- to battleComplete and dumped 9055+9065 together at the kill
		-- gate — the user-visible "tooltips stop after Engaging" bug.
		waitForSignal("playerAttack");
		closeTutorialWidget(player);
		showTutorialSuccessWidget(player, 9055);    -- "Well done! You have successfully executed a battle action."
		openTutorialWidget(player, CONTROLLER_KEYBOARD, TUTORIAL_TP);
		waitForSignal("tpOver1000");
		player:SetMod(modifiersGlobal.MinimumTpLock, 1000);  -- can't dribble below the WS cost mid-lesson
		closeTutorialWidget(player);
		openTutorialWidget(player, CONTROLLER_KEYBOARD, TUTORIAL_WEAPONSKILLS);
		waitForSignal("weaponskillUsed");
		player:SetMod(modifiersGlobal.MinimumTpLock, 0);
		closeTutorialWidget(player);
		showTutorialSuccessWidget(player, 9065);    -- "Well done! You executed a weaponskill."

		waitForSignal("battleComplete");            -- S4.1 fires when all 3 wolves are dead
		-- Render-settle beat: the gate fires in the same call stack as the
		-- third wolf's death broadcast, so without this the defeat dialog
		-- lands in the SAME drain/second as the death packets. Retail
		-- shows the death animation plus a beat before the overlay. (#28.)
		wait(3);
		closeTutorialWidget(player);
	elseif player:IsDiscipleOfMagic() then
		callClientFunction(player, "delegateEvent", player, man0g0Quest, "processTtrBtlMagic001");
		player:EndEvent();
		wait(1);
		kickEventContinue(player, actor, "noticeEvent", "noticeEvent");
		-- The kicked noticeEvent must be ended again: left open, the client
		-- stays in event mode, so F (active mode) and attacks are dead and
		-- the fight can never start. Same targeting lesson + EndEvent as the
		-- DoW branch; it returns once a wolf is targeted, with movement,
		-- hotbar and targeting unlocked.
		callClientFunction(player, "delegateEvent", player, man0g0Quest, "processTtrBtl002", nil, nil, nil);
		player:EndEvent();
		closeTutorialWidget(player);
		openTutorialWidget(player, CONTROLLER_KEYBOARD, TUTORIAL_DEFEATENEMY);
		waitForSignal("battleComplete");
		wait(3);                                    -- render-settle beat (see DoW branch)
		closeTutorialWidget(player);
		showTutorialSuccessWidget(player, 9050);
	end

	worldMaster = GetWorldMaster();
	player:SendDataPacket("attention", worldMaster, "", 51073, 2);
	wait(2);                                        -- decoration only, not load-bearing
	player:ChangeMusic(7);
	player:ChangeState(0);                          -- sheathe → State trio (0x134+0x13C+0x139) flushes

	-- Reopen the event context BEFORE delegating: the post-fight
	-- noticeEvent ended after Btl002, so a bare delegate ships with
	-- owner=0 and the client echo-drops it instantly (live 03:37:40Z:
	-- 020_1 round-tripped in the same second, no cutscene, no item
	-- dialog). The kick parks until the client EventStarts the
	-- noticeEvent (owner=director), and the delegate then runs INSIDE
	-- that open event — the exact working Btl001/Btl002 pattern.
	-- (Garlemald-Server #28, issues 3/5.)
	kickEventContinue(player, actor, "noticeEvent", "noticeEvent");
	-- Fade-out + MAN0G020 + MAN0G030 + the client-side item dialog
	-- (openPublicInformDialogWidget + worldMaster:notify(25117, 11000088
	-- "Treant Vine") — decoded from the shipped Man0g0.lpb) + town mask;
	-- ends with startFadeInCutSceneAfterWarp → EXPECTS the DoZoneChange
	-- right after. The delegate parks until the WHOLE chain completes
	-- client-side (Btl001 parity: its round trip is ~30s).
	callClientFunction(player, "delegateEvent", player, man0g0Quest, "processEvent020_1");
	man0g0Quest:StartSequence(10);
	player:EndEvent();
	player:GetZone():ContentFinished();             -- S1.3 binding + teardown applier
	GetWorldManager():DoZoneChange(player, 155, "PrivateAreaMasterPast", 1, 15, 175.38, -1.21, -1156.51, -2.1);   -- S1.2 arm
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

-- NOTE: this director intentionally defines NO `main` (and no
-- `onCreateContentArea`). pmeteor's QuestDirectorMan0g001 has neither — its
-- `onCreateContentArea` is orphan/dead code never called from C#. The content
-- group's spawning + roster are owned entirely by SimpleContent30010.lua's
-- `onCreate` (director:AddMember ×7), and the content-group wire trio is
-- emitted Rust-side from apply_do_zone_change_content. A `main(director,
-- contentGroup)` here used to crash on a nil `contentGroup` (the engine only
-- passes the director to a director's main), aborting before StartContentGroup
-- with no effect on the wire. (Garlemald-Server #28.)
