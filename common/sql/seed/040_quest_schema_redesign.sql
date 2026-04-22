-- Quest-engine redesign ported from `origin/ioncannon/quest_system`:
--
--   * `characters_quest_completed`: one-row-per-quest → one-row-per-character
--     with the 2048-bit completion bitfield packed into a 256-byte BLOB.
--   * `characters_quest_scenario`: drop the free-form `questData` JSON blob,
--     rename `currentPhase` → `sequence` and `questFlags` → `flags`, and add
--     three 16-bit counters driven by Meteor's `QuestData.SetCounter(...)`.
--
-- Safe to run on existing `data-*Start.zip` save states because the old quest
-- pipeline was 🟡 stub-only (no live producer of scenario rows; no real
-- completion writes), so nothing meaningful is lost by wiping the tables.
-- On a fresh DB the `schema.sql` create just ran with the new shape, so the
-- DROP+CREATE here is effectively a no-op bit-for-bit reshape.

DROP TABLE IF EXISTS characters_quest_completed;
CREATE TABLE characters_quest_completed (
    characterId     INTEGER PRIMARY KEY NOT NULL,
    completedQuests BLOB
);

DROP TABLE IF EXISTS characters_quest_scenario;
CREATE TABLE characters_quest_scenario (
    characterId INTEGER NOT NULL,
    slot        INTEGER NOT NULL,
    questId     INTEGER NOT NULL,
    sequence    INTEGER NOT NULL DEFAULT 0,
    flags       INTEGER NOT NULL DEFAULT 0,
    counter1    INTEGER NOT NULL DEFAULT 0,
    counter2    INTEGER NOT NULL DEFAULT 0,
    counter3    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (characterId, slot)
);
