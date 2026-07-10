-- Man0u1 "Court in the Sands" — SEQ_050/057 emote-mechanic conditions
-- (Garlemald-Server #53; the seed/080 man0g1 dance-drill recipe).
--
-- A player emote only fires the quest onEmote hook when the TARGET
-- actor's gamedata_actor_class carries an emoteEventConditions entry
-- whose emoteId matches the emote performed (seed/080 root-cause;
-- otherwise the client routes it to the free EmoteStandardCommand and
-- the quest never hears it). man0u1's teach/calm mechanic needs all SIX
-- taught emotes on F'lhaminn (teach phase) AND both miners (calm
-- phase): Furious 103, Beckon 108, Laugh 121, Deny 125, Upset 140,
-- Soothe 135 — the 100-range EmoteStandardCommand ids, order-locked to
-- emoteDefault1..6 because man0u1.lua's EMOTE_STEPS table indexes the
-- conditionName suffix (Maddened wants emoteDefault2, Manic wants
-- 6→1→3). Emote-id evidence: the client-side F'lhaminn demo schedulers
-- encode exactly these ids (0x5000000 | ((id-100) << 12)); the six-id
-- set is verbatim in the pmeteor header table.
--
-- REPLACED conditions note: all three classes ship with NULL
-- eventConditions in seed/003 AND both upstreams (nothing load-bearing
-- existed); seed/093's crash-safety fill gives them the plain
-- talkDefault + noticeEvent baseline, which this seed re-states inside
-- the superset — the talks (processEvent051_x / 050_13 / 050_14) keep
-- working and the names light as quest targets. Untaught emotes match
-- no condition and simply never fire (retail behaviour).
--
-- Idempotent: the UPDATE converges (same JSON on re-run). No schema change.

UPDATE "gamedata_actor_class" SET "eventConditions" =
'{"talkEventConditions":[{"unknown1":4,"unknown2":0,"conditionName":"talkDefault"}],"noticeEventConditions":[{"unknown1":0,"unknown2":1,"conditionName":"noticeEvent"}],"emoteEventConditions":[{"unknown1":4,"unknown2":0,"emoteId":103,"conditionName":"emoteDefault1"},{"unknown1":4,"unknown2":0,"emoteId":108,"conditionName":"emoteDefault2"},{"unknown1":4,"unknown2":0,"emoteId":121,"conditionName":"emoteDefault3"},{"unknown1":4,"unknown2":0,"emoteId":125,"conditionName":"emoteDefault4"},{"unknown1":4,"unknown2":0,"emoteId":140,"conditionName":"emoteDefault5"},{"unknown1":4,"unknown2":0,"emoteId":135,"conditionName":"emoteDefault6"}]}'
WHERE "id" IN (
    1000842, -- F'lhaminn (scene/teach variant — SEQ_050 accepts all six in teach order)
    1001283, -- Manic Miner (SEQ_057 wants Soothe→Furious→Laugh = emoteDefault6→1→3)
    1001284  -- Maddened Miner (SEQ_057 wants Beckon = emoteDefault2)
);
