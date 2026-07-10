-- Man0u1 "Court in the Sands" — content battle-npc data (Garlemald-Server
-- #53): the SEQ_015 Coliseum match (SimpleContentMan0u101 spawns bnpc
-- 42) and the SEQ_060 F'lhaminn escort (SimpleContentMan0u102 spawns
-- bnpc 43 + chinchillas 44-51). Mirrors seed/074's trigger+escort bnpc
-- sections and seed/082's kid-class recipe.
--
--   (1) Tourney Gladiator (class 2280157, display 3280156 — bahamut
--       gamedata; humanoid, appearance row shipped): the UNWINNABLE 1v1
--       opponent. Retail (both full playthroughs + the beta uploader's
--       notes): hits a level-1-3 player for 25-79 (Light Stab / Heavy
--       Stab / Red Lotus on film) and cannot die — the content script
--       MinimumHpLocks him; losing IS completing. Class ships stripped
--       (classPath '') in both trees — fill with the PROVEN
--       FighterAllyOpeningAttacker kit (its class script supplies the
--       battleCommon tuple; a battle npc without one crashes the
--       client's DepictionJudge — the man0l1 chigoe / seed/082 lesson;
--       "Ally" is only the classPath name, hostility comes from the
--       group's allegiance 0 + ChangeState(2) in-script). Level 5 per
--       the 053/055 pacing rationale (a level-1-3 player must LOSE the
--       ~25-79-damage exchange); HP is cosmetic under MinimumHpLock.
--       DAMAGE BAND FLAGGED for in-client tuning against the retail
--       25-79 window.
--   (2) F'lhaminn escortee: the seed/082 recipe verbatim — the game
--       already ships an ally-band class 2290008 carrying her REAL
--       display name (1900054) and an appearance row byte-identical to
--       her scene class 1000842, merely stripped. Fill classPath only;
--       no appearance clone needed (full retail fidelity, zero render
--       risk). Allegiance 1 like Sisipu/Powle (excluded from live
--       hostile counts); 200 HP (the kids' band) — she takes real
--       damage, her wounding is the retail fail condition.
--   (3) Chinchillas (class 2204010, display 3204010 — the retail combat
--       log names "chinchilla" throughout): appearance row shipped
--       (model 10502 = the Lemming family; wharf rat 2104001 carries
--       LemmingStandard on the same model), class stripped — fill with
--       LemmingStandard (the seed/056 chigoe family-model recipe).
--       Genus 12 'Rodent' per the same wharf-rat precedent. Weak per
--       the seed/074 biter band: level 5-7, 60 HP (retail: 60-80-damage
--       weaponskills one-shot them, 511-650 EXP each on film).
--
-- Ids: pools 15-17, groups 16-18, bnpc spawn rows 42-51 — all next-free
-- (Sisipu's escort used bnpc 25-33, man0g1's 34-41; pools/groups topped
-- out at 14/15 in seed/074). groupId.zoneId is metadata only (the live
-- spawn zone comes from the content area's parent zone) but is kept
-- agreeing with where each duty runs (209 arena / 170 escort).
--
-- Chinchilla coordinates are ROUTE-INTERPOLATED, not captured (no
-- surviving 1.x recording renders XYZ): three clusters on the straight
-- line gate (-34.5, 183, -68.9) → camp (34.8, 201, -480.5) at t≈0.3 /
-- 0.6 / 0.85 of the run, ±4-unit offsets, y linear (183→201) — FLAG for
-- the RECORD_MODE re-capture pass the siblings got (#41/#46). The arena
-- anchor (-205.4, 192.0, 150.9) is the GC-quest coliseum marker
-- (11092101), the only in-data arena anchor — y FLAGGED for in-client
-- tuning.
--
-- Idempotent: UPDATEs guarded, INSERTs OR IGNORE. No schema change.

-- ---- (1) classPath fills ---------------------------------------------

-- Tourney Gladiator — humanoid battle npc on the proven Fighter kit.
UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Monster/Fighter/FighterAllyOpeningAttacker'
WHERE "id" = 2280157 AND "classPath" = '';

-- F'lhaminn's shipped ally-band class (seed/082 recipe).
UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Monster/Fighter/FighterAllyOpeningAttacker'
WHERE "id" = 2290008 AND "classPath" = '';

-- Chinchilla — Lemming family model path (seed/056 chigoe recipe).
UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Monster/Lemming/LemmingStandard'
WHERE "id" = 2204010 AND "classPath" = '';

-- ---- (2) Pools --------------------------------------------------------

-- Gladiator: genus 29 'Person', currentJob 3 (GLA — cosmetic; the melee
-- spawn path treats melee jobs identically, the 054 caster-note analog).
INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (15, 2280157, 'tourney_gladiator', 29, 3, 1, 4200, 1, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (16, 2290008, 'escort_flhaminn', 29, 2, 1, 4200, 1, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (17, 2204010, 'chinchilla', 12, 0, 1, 4200, 1, 0, 0, 0, 0, 0);

-- ---- (3) Groups -------------------------------------------------------

-- Gladiator: hostile (allegiance 0), level 5, HP cosmetic under the
-- in-script MinimumHpLock.
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (16, 15, 'tourney_gladiator', 5, 5, 0, 1000, 0, 0, 0, 1, 0, 0, '', 0, 209);
-- F'lhaminn: ally (allegiance 1), kids'-band HP — her wounding fails
-- the duty (content-script live-roster check).
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (17, 16, 'escort_flhaminn', 1, 1, 0, 200, 0, 0, 1, 1, 0, 0, '', 0, 170);
-- Chinchillas: the man0l1/man0g1 biter band (lvl 5-7, hp 60 — one-shot
-- at opener weaponskill damage, per the retail recording).
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (18, 17, 'chinchilla', 5, 7, 0, 60, 0, 0, 0, 1, 0, 0, '', 0, 170);

-- ---- (4) Spawn points (SimpleContentMan0u101/102 spawn these ids) -----

-- The bloodsands anchor (GC-quest coliseum marker 11092101; y FLAGGED).
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (42, 'tourney_gladiator', 16, -205.4, 192.0, 150.9, 0.0);

-- F'lhaminn beside the gate warp-in (the escort starts with her in tow).
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (43, 'escort_flhaminn', 17, -31.0, 183.0, -66.0, 2.0);

-- Chinchilla cluster A — route point t≈0.3 (2 mobs). ROUTE-INTERPOLATED.
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (44, 'chinchilla', 18, -17.7, 188.4, -190.4, 0.0);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (45, 'chinchilla', 18, -9.7, 188.4, -194.4, 0.0);

-- Chinchilla cluster B — route point t≈0.6 (3 mobs). ROUTE-INTERPOLATED.
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (46, 'chinchilla', 18, 3.1, 193.8, -313.9, 0.0);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (47, 'chinchilla', 18, 11.1, 193.8, -317.9, 0.0);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (48, 'chinchilla', 18, 7.1, 193.8, -311.9, 0.0);

-- Chinchilla cluster C — route point t≈0.85 (3 mobs). ROUTE-INTERPOLATED.
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (49, 'chinchilla', 18, 20.4, 198.3, -416.8, 0.0);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (50, 'chinchilla', 18, 28.4, 198.3, -420.8, 0.0);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (51, 'chinchilla', 18, 24.4, 198.3, -422.8, 0.0);
