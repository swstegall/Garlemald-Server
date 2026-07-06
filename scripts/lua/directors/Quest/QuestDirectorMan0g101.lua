require ("global")
require ("quests/man/man0g1")

-- Man0g1 SEQ_065 White Wolf Gate escort director (Garlemald-Server
-- #41) — clone of the proven QuestDirectorMan0l101 completion-beat
-- coroutine (every ordering rule below is wire-proven on the man0l1
-- leg; see that file for the receipts):
--
--   client noticeEvent kick (startMan0g1Content's KickEvent) →
--   onEventStarted dismisses the staging veil (questBaseRewardSeting =
--   the ONLY Now-Loading dismissal after a kick-at-entry; a bare
--   EndEvent leaves the client veiled forever) → EndEvent → parks on
--   "escortComplete" → the content script signals arrival (Powle led
--   the player to the Lifemend Stump) → arrival cutscene
--   (processEvent180 — retail's escort End Cutscene, the beat the old
--   stub played WITHOUT the duty) → SEQ_070 journal update + unbind →
--   ContentFinished teardown → warp into the stump scene private area.
--
--   The content script ALSO fires "escortComplete" on its FAIL flow
--   purely to DRAIN this park (a stale park would double-fire the
--   arrival on a retry). In that case the player was already rolled
--   back to SEQ_060 and warped out, so the kickEventContinue below
--   emits a kick for a wiped director actor — the client drops it,
--   this coroutine parks on _WAIT_EVENT and is later displaced, and
--   the StartSequence(SEQ_070) tail never runs.

function init()
	return "/Director/Quest/QuestDirectorMan0g101";
end

function onEventStarted(player, actor, triggerName)
	man0g1Quest = player:GetQuest("Man0g1");

	-- Post-warp Now-Loading DISMISSAL (the man0l1 round-7c
	-- decomp-final finding): the kicked noticeEvent arms a staging
	-- veil whose only dismissal is a client Lua call to MyPlayer slot
	-- 66 — questBaseRewardSeting's client body is exactly that
	-- (_fadeInNowLoadingForNoticeEventJustInArea +
	-- startFadeInCutSceneDefault + _wait(0.5)).
	callClientFunction(player, "delegateEvent", player, man0g1Quest, "questBaseRewardSeting");

	-- NO entry text here — the duty banners (protect/bound/timer) ride
	-- the content script's first-sighting latch (it alone holds Powle's
	-- actor id for the 51005 param). 34108 "You have entered an
	-- instance." rides the Rust content-warp path.

	-- Close the kick — post-load, like the tutorials.
	player:EndEvent();

	waitForSignal("escortComplete");

	-- Render-settle beat (Man0l001 pattern): without it the arrival
	-- cutscene lands in the same drain as the last death packets.
	wait(2);

	-- Reopen the event context BEFORE delegating — a bare delegate
	-- ships with owner=0 and the client echo-drops it.
	kickEventContinue(player, actor, "noticeEvent", "noticeEvent");
	-- Retail's escort End Cutscene ("warps into the Echo" — the
	-- Hermit-of-the-Wood confrontation). Ends by arming an after-warp
	-- veil — it MUST be followed by the private-area warp below, whose
	-- real load resolves the veil.
	callClientFunction(player, "delegateEvent", player, man0g1Quest, "processEvent180");
	-- Stump handoff (retail order: journal update → 34108 → unbind).
	-- StartSequence(SEQ_070) IS the journal update; 34108 rides the
	-- Rust private-area warp path. The unbind line goes out BEFORE the
	-- EndEvent + warp (a 0x0157 game message queued after the warp
	-- ships into the client's Now-Loading gap — the man0l0 Hob-crash
	-- shape). 50012 = "You are no longer bound by duty." (sheet-decoded
	-- worldMaster duty block; on film for this duty at the stump).
	man0g1Quest:StartSequence(SEQ_070);
	player:SendGameMessage(GetWorldMaster(), 50012, 0x20);
	player:EndEvent();
	player:GetZone():ContentFinished();
	-- The stump scene: the ported SEQ_070 beats live in
	-- "PrivateAreaMasterPast" level 0 at pmeteor's (-770.197, 23,
	-- -1086.209) — geographically the Lifemend Stump (wiki marker zone
	-- 150 ≈ (-800, 20, -1050)), so the private area is addressed on
	-- ZONE 150 (the duty's parent zone; upstream's town-zone
	-- WarpToPrivateArea ran in a dead path and was never exercised).
	-- If 150 lacks the named private area, do_zone_change falls back
	-- to the parent core at the same coords (world_manager warn arm) —
	-- flagged for the live-verify round in #41.
	GetWorldManager():DoZoneChange(player, 150, "PrivateAreaMasterPast", 0, 15, -770.197, 23, -1086.209, 0.0);
end

function main()
end
