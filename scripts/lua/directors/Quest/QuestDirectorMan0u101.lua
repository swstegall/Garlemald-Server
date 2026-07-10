require ("global")
require ("quests/man/man0u1")

-- Man0u1 SEQ_015 Coliseum first-round match director (Garlemald-Server
-- #53) — the QuestDirectorMan0l101/Man0g101 completion-beat coroutine
-- (every ordering rule is wire-proven on those legs), adapted to the
-- arena's lose-to-complete shape:
--
--   client noticeEvent kick (startMan0u1Coliseum's KickEvent) →
--   onEventStarted dismisses the staging veil (questBaseRewardSeting =
--   the ONLY Now-Loading dismissal after a kick-at-entry) → EndEvent →
--   parks on "coliseumFought" → the content script signals the loss
--   (the fight is unwinnable by design — Mirke: "There is no way to
--   win.") → post-match cutscene (processEvent035, retail CS5 — "A
--   splendid match! I expected no less. / Don't let losing get to
--   you.") → GLD counter 3 → 4 → ContentFinished teardown → warp into
--   the Coliseum echo PA, where Greinfarr's talk plays the "gathering"
--   echo (processEvent040) and hands back to the Quicksand.
--
--   The content script has no fail flow — the loss IS completion; a
--   session death mid-fight is covered by the quest script's
--   FLAG_ARENA_HANDOFF relog rescue (GLD 3 → 2, trigger re-arms).

function init()
	return "/Director/Quest/QuestDirectorMan0u101";
end

function onEventStarted(player, actor, triggerName)
	man0u1Quest = player:GetQuest("Man0u1");

	-- Post-warp Now-Loading dismissal (MyPlayer slot 66 — the man0l1
	-- round-7c decomp-final finding).
	callClientFunction(player, "delegateEvent", player, man0u1Quest, "questBaseRewardSeting");

	-- Close the kick — post-load, like the tutorials. The duty-bound
	-- banner rides the content script's first-sighting latch.
	player:EndEvent();

	waitForSignal("coliseumFought");

	-- Render-settle beat (Man0l001 pattern).
	wait(2);

	-- Reopen the event context BEFORE delegating — a bare delegate
	-- ships with owner=0 and the client echo-drops it.
	kickEventContinue(player, actor, "noticeEvent", "noticeEvent");
	-- man0u135: the post-match Greinfarr scene (retail CS5). Ends by
	-- arming an after-warp veil — the echo-PA warp below resolves it.
	callClientFunction(player, "delegateEvent", player, man0u1Quest, "processEvent035");
	-- Fight done: GLD 3 → 4 (the echo-PA Greinfarr talk gates on 4).
	-- The unbind line goes out BEFORE the EndEvent + warp (a 0x0157
	-- game message queued after the warp ships into the Now-Loading
	-- gap — the man0l0 Hob-crash shape). 50012 = "You are no longer
	-- bound by duty." — on film right at this transition.
	man0u1Quest:GetData():SetCounter(CNTR_SEQ15_GLD, 4);
	-- SetCounter does NOT re-run onStateChange (only StartSequence/
	-- UpdateENPCs do), and the echo-PA zone-in replays the LAST ENPC
	-- snapshot — without this re-derivation Greinfarr lands un-iconed
	-- in the echo scene (QFLAG_TALK gates on GLD==4). Branch review
	-- finding, 2026-07-09.
	man0u1Quest:UpdateENPCs();
	player:SendGameMessage(GetWorldMaster(), 50012, 0x20);
	player:EndEvent();
	player:GetZone():ContentFinished();
	-- The post-match echo scene PA (Greinfarr + Niellefresne cast,
	-- seed/094) — same lobby PA at the Coliseum coords.
	GetWorldManager():DoZoneChange(player, 209, "PrivateAreaMasterPast", 5, 15, -185.0, 195.0, 190.0, -2.3);
end

function main()
end
