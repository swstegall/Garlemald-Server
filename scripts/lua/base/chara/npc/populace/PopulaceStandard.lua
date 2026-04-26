
function init(npc)
	return false, false, 0, 0;
end

-- Default onEventStarted is intentionally a no-op. The Rust-side
-- `handle_event_start` does its own per-quest fan-out by `event_type`
-- (1→onTalk, 2→onPush, 3→onEmote, 0→onCommand) AFTER this base script
-- runs, and the quest's own hook is what calls `player:EndEvent()` at
-- the tail of its cinematic chain. Doing an EndEvent here would clear
-- the client's event before the quest's `callClientFunction(...)` →
-- `RunEventFunction` packet arrives, which silently drops the cinematic
-- (visible symptom: walking up to Yda after the opening cinematic does
-- nothing on screen even though `man0g0::onPush` runs server-side).
--
-- TODO: when no quest hook handles the event (e.g. default-talk against
-- an NPC with no active quest), the client stays event-locked. The Rust
-- side will need a "no quest fired, EndEvent fallback" pass for that
-- case once default-talk dialog menus are wired up.
function onEventStarted(player, npc)
end

function onEventUpdate(player, npc, blah, menuSelect)
	player:EndEvent();
end