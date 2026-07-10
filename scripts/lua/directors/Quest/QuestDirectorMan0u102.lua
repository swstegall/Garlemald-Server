require ("global")
require ("quests/man/man0u1")

-- Man0u1 SEQ_060 F'lhaminn escort director (Garlemald-Server #53) —
-- clone of the proven QuestDirectorMan0l101/Man0g101 completion-beat
-- coroutine (every ordering rule is wire-proven on those legs):
--
--   client noticeEvent kick (startMan0u1Escort's KickEvent) →
--   onEventStarted dismisses the staging veil (questBaseRewardSeting)
--   → EndEvent → parks on "man0u1EscortComplete" → the content script
--   signals arrival at Camp Black Brush → arrival cutscene
--   (processEvent075 — man0u175; retail: "Thank you. We have arrived
--   in Camp Black Brush.") → SEQ_065 journal update + unbind →
--   ContentFinished teardown → drop to PUBLIC zone 170 at the camp
--   (no client-provable PA exists for wil0Fld01 — the Ascilia echo
--   cast is SetENpc-gated there instead; design-doc checkpoint 4).
--
--   The content script ALSO fires "man0u1EscortComplete" on its FAIL
--   flow purely to DRAIN this park (a stale park would double-fire the
--   arrival on a retry). In that case the sequence is still SEQ_060 —
--   this coroutine's kickEventContinue targets a wiped director actor,
--   the client drops it, and the tail never runs.

function init()
	return "/Director/Quest/QuestDirectorMan0u102";
end

function onEventStarted(player, actor, triggerName)
	man0u1Quest = player:GetQuest("Man0u1");

	-- Post-warp Now-Loading dismissal (MyPlayer slot 66).
	callClientFunction(player, "delegateEvent", player, man0u1Quest, "questBaseRewardSeting");

	-- Close the kick — post-load. The duty banners (protect/bound/
	-- timer) ride the content script's first-sighting latch (it alone
	-- holds F'lhaminn's actor id for the 51005 param).
	player:EndEvent();

	waitForSignal("man0u1EscortComplete");

	-- Render-settle beat.
	wait(2);

	kickEventContinue(player, actor, "noticeEvent", "noticeEvent");
	-- man0u175: the camp-arrival cutscene. Ends by arming an
	-- after-warp veil — the public drop below resolves it.
	callClientFunction(player, "delegateEvent", player, man0u1Quest, "processEvent075");
	-- Camp handoff (retail order: journal update → unbind → reload).
	man0u1Quest:StartSequence(SEQ_065);
	player:SendGameMessage(GetWorldMaster(), 50012, 0x20);
	player:EndEvent();
	player:GetZone():ContentFinished();
	-- Camp Black Brush, public zone 170 (camp anchor y=201 from the
	-- seed-031 camp rows; the marker sheet puts the leader beat at
	-- 41.1/-480.0).
	GetWorldManager():DoZoneChange(player, 170, nil, 0, 15, 34.8, 201.0, -480.5, -1.5);
end

function main()
end
