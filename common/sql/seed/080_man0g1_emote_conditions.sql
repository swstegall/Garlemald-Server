-- Man0g1 SEQ_050 dance drill — THE emote-mechanic fix (Garlemald-Server
-- #41, live 2026-07-06: "clapping for Nicollaux does nothing"). The user
-- pointed at man0l1's working Sisipu-emote step as the template.
--
-- Root cause: a player emote only fires the quest onEmote hook
-- (processor.rs: EventStart event_type 3 -> fire_quest_hook_for_active_
-- quests "onEmote") when the TARGET actor's gamedata_actor_class
-- carries an `emoteEventConditions` entry whose emoteId matches the
-- emote performed. man0l1's dedicated SISIPU_EMOTE class (1000155) has
-- exactly that block (emoteId 105/107/129/128/118/116 -> emoteDefault1..6);
-- without it the client routes the emote to the free EmoteStandardCommand
-- (log: "EmoteStandardCommand onEventStarted fired") and the quest never
-- hears it. seed/076/078 gave the kids only talkDefault + noticeEvent, so
-- the drill could never register a single emote.
--
-- Fix: each kid learns ONE emote (retail Part-4; matches man0g1.lua
-- onEmote's DoEmote calls). Give each kid class a single
-- emoteEventCondition for THAT emote's command id (the 100-range emoteId
-- from EmoteStandardCommand.lua's emoteTable), conditionName
-- emoteDefault1 (man0g1 onEmote branches on classId + the DONE flag, not
-- the name -- a single condition per kid is enough, and doing the WRONG
-- emote at a kid simply matches no condition, so nothing fires -- retail
-- behaviour). talkDefault + noticeEvent preserved so the "teach" talks
-- still work and the names light as quest targets.
--   Ryd 1000412  -> Surprised 101   Aunillie 1000410 -> Beckon 108
--   Nicollaux 1000409 -> Clap 107    Sansa 1000239  -> Bow 105
--   Powle 1000238 -> Cheer 106       Elyn 1000411   -> Lookout 122
--
-- These kid classes are shared with the etc5g1 scene; a stray onEmote
-- there matches no active quest and no-ops, so this is safe.

UPDATE "gamedata_actor_class" SET "eventConditions" =
'{"talkEventConditions":[{"unknown1":4,"unknown2":0,"conditionName":"talkDefault"}],"noticeEventConditions":[{"unknown1":0,"unknown2":1,"conditionName":"noticeEvent"}],"emoteEventConditions":[{"unknown1":4,"unknown2":0,"emoteId":101,"conditionName":"emoteDefault1"}]}'
WHERE "id" = 1000412;

UPDATE "gamedata_actor_class" SET "eventConditions" =
'{"talkEventConditions":[{"unknown1":4,"unknown2":0,"conditionName":"talkDefault"}],"noticeEventConditions":[{"unknown1":0,"unknown2":1,"conditionName":"noticeEvent"}],"emoteEventConditions":[{"unknown1":4,"unknown2":0,"emoteId":108,"conditionName":"emoteDefault1"}]}'
WHERE "id" = 1000410;

UPDATE "gamedata_actor_class" SET "eventConditions" =
'{"talkEventConditions":[{"unknown1":4,"unknown2":0,"conditionName":"talkDefault"}],"noticeEventConditions":[{"unknown1":0,"unknown2":1,"conditionName":"noticeEvent"}],"emoteEventConditions":[{"unknown1":4,"unknown2":0,"emoteId":107,"conditionName":"emoteDefault1"}]}'
WHERE "id" = 1000409;

UPDATE "gamedata_actor_class" SET "eventConditions" =
'{"talkEventConditions":[{"unknown1":4,"unknown2":0,"conditionName":"talkDefault"}],"noticeEventConditions":[{"unknown1":0,"unknown2":1,"conditionName":"noticeEvent"}],"emoteEventConditions":[{"unknown1":4,"unknown2":0,"emoteId":105,"conditionName":"emoteDefault1"}]}'
WHERE "id" = 1000239;

UPDATE "gamedata_actor_class" SET "eventConditions" =
'{"talkEventConditions":[{"unknown1":4,"unknown2":0,"conditionName":"talkDefault"}],"noticeEventConditions":[{"unknown1":0,"unknown2":1,"conditionName":"noticeEvent"}],"emoteEventConditions":[{"unknown1":4,"unknown2":0,"emoteId":106,"conditionName":"emoteDefault1"}]}'
WHERE "id" = 1000238;

UPDATE "gamedata_actor_class" SET "eventConditions" =
'{"talkEventConditions":[{"unknown1":4,"unknown2":0,"conditionName":"talkDefault"}],"noticeEventConditions":[{"unknown1":0,"unknown2":1,"conditionName":"noticeEvent"}],"emoteEventConditions":[{"unknown1":4,"unknown2":0,"emoteId":122,"conditionName":"emoteDefault1"}]}'
WHERE "id" = 1000411;

-- Tighter cluster on the KNOWN-FLAT ground next to the player's spawn
-- (the wider -238..-210 spread from seed/079 straddled a fence/ravine
-- in the PA1 geometry and re-buried Nicollaux). Keep every kid within
-- ~6u of the anchor (-223.792, 12, -1498.369) where the player stands
-- fine, facing the player (rotation ~1.57 = +X). PROVISIONAL -- precise
-- placement still wants a live stand-and-mark (read the player's 0x00CA
-- at each desired spot). Retail semicircle order preserved L->R.
UPDATE "server_spawn_locations" SET "positionX" = -229.0, "positionY" = 12.0, "positionZ" = -1494.0, "rotation" = 1.90 WHERE "uniqueId" = 'man0g1_ryd_drill';
UPDATE "server_spawn_locations" SET "positionX" = -230.0, "positionY" = 12.0, "positionZ" = -1496.0, "rotation" = 1.70 WHERE "uniqueId" = 'man0g1_aunillie_drill';
UPDATE "server_spawn_locations" SET "positionX" = -230.0, "positionY" = 12.0, "positionZ" = -1498.0, "rotation" = 1.57 WHERE "uniqueId" = 'man0g1_sansa_drill';
UPDATE "server_spawn_locations" SET "positionX" = -230.0, "positionY" = 12.0, "positionZ" = -1500.0, "rotation" = 1.44 WHERE "uniqueId" = 'man0g1_powle_drill';
UPDATE "server_spawn_locations" SET "positionX" = -229.0, "positionY" = 12.0, "positionZ" = -1502.0, "rotation" = 1.24 WHERE "uniqueId" = 'man0g1_elyn_drill';
UPDATE "server_spawn_locations" SET "positionX" = -228.0, "positionY" = 12.0, "positionZ" = -1503.0, "rotation" = 1.10 WHERE "uniqueId" = 'man0g1_nicollaux_drill';
UPDATE "server_spawn_locations" SET "positionX" = -226.0, "positionY" = 12.0, "positionZ" = -1499.0, "rotation" = -1.57 WHERE "uniqueId" = 'man0g1_fufucha_drill';
