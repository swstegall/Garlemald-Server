-- Ported from project-meteor-mirror/Data/sql/server_battlenpc_pools.sql
-- Table: server_battlenpc_pools

CREATE TABLE IF NOT EXISTS "server_battlenpc_pools" (
    "poolId" INTEGER NOT NULL,
    "actorClassId" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    "genusId" INTEGER NOT NULL,
    "currentJob" INTEGER NOT NULL DEFAULT '0',
    "combatSkill" INTEGER NOT NULL,
    "combatDelay" INTEGER NOT NULL,
    "combatDmgMult" REAL unsigned NOT NULL DEFAULT '1',
    "aggroType" INTEGER NOT NULL DEFAULT '0',
    "immunity" INTEGER NOT NULL DEFAULT '0',
    "linkType" INTEGER NOT NULL DEFAULT '0',
    "spellListId" INTEGER NOT NULL DEFAULT '0',
    "skillListId" INTEGER NOT NULL DEFAULT '0',
    PRIMARY KEY ("poolId")
);

INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (1, 2104001, 'wharf_rat', 12, 0, 1, 4200, 1, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (2, 2201407, 'bloodthirsty_wolf', 3, 0, 1, 4200, 1, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (3, 2290005, 'yda', 29, 2, 1, 4200, 1, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (4, 2290006, 'papalymo', 29, 22, 1, 4200, 1, 0, 0, 0, 0, 0);
