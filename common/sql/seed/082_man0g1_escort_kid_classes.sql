-- Man0g1 "Souls Gone Wild" — FIX the SEQ_065 escortee spawns (Garlemald-Server #41).
--
-- Live crash root cause (2026-07-07 00:35:41, White Wolf Gate → Lifemend
-- Stump duty): the client's Wine render thread froze the instant the
-- zone-150 content instance streamed in (server drove the escort ~18s
-- against a dead client, then TCP RST). Cause: seed/074 tried to CREATE
-- the escort Powle/Sansa as NEW battle-npc-ally classes 2290020/2290021
-- via INSERT OR IGNORE — but those ids ALREADY EXIST in base gamedata as
-- OTHER stripped NPCs (displayNameId 1500004 / 4000421), so the INSERT
-- silently no-opped (the "INSERT OR IGNORE no-ops on collision" gotcha,
-- #46). The escort pool JOIN therefore resolved a class with an EMPTY
-- classPath, and a battle npc with no class script streams via the
-- 0x00CC populace fallback (numBattleCommon=0); the client's
-- DepictionJudge dereferences the nil battleCommon.aggro and dies — the
-- same crash class as the man0l1 chigoe ("classPath-less spawns crash
-- the client").
--
-- The game already ships Powle and Sansa as ally-band classes carrying
-- the CORRECT child NAMES and APPEARANCES, exactly like Sisipu (2290007):
--   2290009 → displayNameId 1000029 "Powle", appearance == populace 1000238
--   2290010 → displayNameId 1100025 "Sansa", appearance == populace 1000239
-- (both appearance rows are byte-identical to the kids' populace looks —
-- base 21 child model). They are merely stripped (classPath=''). Fill
-- them with the proven FighterAlly kit (the Sisipu UPDATE recipe from
-- seed/056) and repoint the escort pools onto them. No appearance clone
-- is needed — the real rows are already the kids' look, so this is full
-- retail fidelity with zero render risk. The bogus 2290020/2290021 rows
-- seed/074's INSERTs targeted are left untouched (they belong to unrelated
-- NPCs; nothing else references them once the pools are repointed).
--
-- Idempotent: the UPDATEs guard on classPath='' / the old actorClassId, so
-- a re-run — or a fresh DB where a later port already fixed these — is a
-- no-op. No schema change.

-- ---- (1) Fill the real kids' ally classes (Sisipu 2290007 pattern) ----

UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Monster/Fighter/FighterAllyOpeningAttacker'
WHERE "id" = 2290009 AND "classPath" = '';

UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Monster/Fighter/FighterAllyOpeningAttacker'
WHERE "id" = 2290010 AND "classPath" = '';

-- ---- (2) Repoint the escort pools onto the real Powle/Sansa classes ----
-- seed/074 built pools 13/14 pointing at the wrong-NPC classes
-- 2290020/2290021. bnpc 34/35 → group 13/14 → pool 13/14 → class; with the
-- pool repointed the spawn JOIN now carries FighterAlly + the kids' names +
-- their child appearances.

UPDATE "server_battlenpc_pools" SET "actorClassId" = 2290009
WHERE "poolId" = 13 AND "actorClassId" = 2290020;

UPDATE "server_battlenpc_pools" SET "actorClassId" = 2290010
WHERE "poolId" = 14 AND "actorClassId" = 2290021;
