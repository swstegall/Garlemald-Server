-- Per-quest NpcLs scratchpad — adds the two C# `QuestData` fields that
-- back `Quest.NewNpcLsMsg` / `ReadNpcLsMsg` / `EndOfNpcLsMsgs`. C#
-- stored these inside the JSON `questData` blob (now removed); garlemald
-- carries them as flat columns alongside the existing
-- sequence/flags/counter1/2/3 columns.
--
-- npc_ls_from     — id (1..=40) of the NPC linkshell currently driving
--                   this quest's message chain. 0 = no chain active.
-- npc_ls_msg_step — 0-based message-step counter, incremented by
--                   `ReadNpcLsMsg` between successive lines of the same
--                   chain. Cleared on `EndOfNpcLsMsgs`.
--
-- See `LuaQuestHandle::{NewNpcLsMsg, ReadNpcLsMsg, EndOfNpcLsMsgs}` in
-- `map-server/src/lua/userdata.rs` for the call site, and the matching
-- C# port at `Quest.cs` (project-meteor-server master branch).

ALTER TABLE characters_quest_scenario ADD COLUMN npc_ls_from INTEGER NOT NULL DEFAULT 0;
ALTER TABLE characters_quest_scenario ADD COLUMN npc_ls_msg_step INTEGER NOT NULL DEFAULT 0;
