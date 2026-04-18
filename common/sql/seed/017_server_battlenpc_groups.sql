-- Ported from project-meteor-mirror/Data/sql/server_battlenpc_groups.sql
-- Table: server_battlenpc_groups

CREATE TABLE IF NOT EXISTS "server_battlenpc_groups" (
    "groupId" INTEGER NOT NULL DEFAULT '0',
    "poolId" INTEGER NOT NULL DEFAULT '0',
    "scriptName" TEXT NOT NULL,
    "minLevel" INTEGER NOT NULL DEFAULT '1',
    "maxLevel" INTEGER NOT NULL DEFAULT '1',
    "respawnTime" INTEGER NOT NULL DEFAULT '10',
    "hp" INTEGER NOT NULL DEFAULT '0',
    "mp" INTEGER NOT NULL DEFAULT '0',
    "dropListId" INTEGER NOT NULL DEFAULT '0',
    "allegiance" INTEGER NOT NULL DEFAULT '0',
    "spawnType" INTEGER NOT NULL DEFAULT '0',
    "animationId" INTEGER NOT NULL DEFAULT '0',
    "actorState" INTEGER NOT NULL DEFAULT '0',
    "privateAreaName" TEXT NOT NULL DEFAULT '',
    "privateAreaLevel" INTEGER NOT NULL DEFAULT '0',
    "zoneId" INTEGER NOT NULL,
    PRIMARY KEY ("groupId")
);

INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (1, 1, 'wharf_rat', 1, 1, 10, 0, 0, 0, 0, 0, 0, 0, '', 0, 170);
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (2, 2, 'bloodthirsty_wolf', 1, 1, 0, 0, 0, 0, 0, 1, 0, 0, '', 0, 166);
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (3, 3, 'yda', 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, '', 0, 166);
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (4, 4, 'papalymo', 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, '', 0, 166);
