-- Ported from project-meteor-mirror/Data/sql/server_battlenpc_pool_mods.sql
-- Table: server_battlenpc_pool_mods

CREATE TABLE IF NOT EXISTS "server_battlenpc_pool_mods" (
    "poolId" INTEGER NOT NULL,
    "modId" INTEGER NOT NULL,
    "modVal" INTEGER NOT NULL,
    "isMobMod" INTEGER NOT NULL DEFAULT '0',
    PRIMARY KEY ("poolId", "modId")
);

INSERT OR IGNORE INTO "server_battlenpc_pool_mods" ("poolId", "modId", "modVal", "isMobMod") VALUES
    (2, 2, 3, 1);
INSERT OR IGNORE INTO "server_battlenpc_pool_mods" ("poolId", "modId", "modVal", "isMobMod") VALUES
    (2, 3, 3, 1);
INSERT OR IGNORE INTO "server_battlenpc_pool_mods" ("poolId", "modId", "modVal", "isMobMod") VALUES
    (2, 24, 0, 1);
INSERT OR IGNORE INTO "server_battlenpc_pool_mods" ("poolId", "modId", "modVal", "isMobMod") VALUES
    (3, 24, 0, 1);
INSERT OR IGNORE INTO "server_battlenpc_pool_mods" ("poolId", "modId", "modVal", "isMobMod") VALUES
    (3, 49, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_pool_mods" ("poolId", "modId", "modVal", "isMobMod") VALUES
    (4, 24, 0, 1);
INSERT OR IGNORE INTO "server_battlenpc_pool_mods" ("poolId", "modId", "modVal", "isMobMod") VALUES
    (4, 49, 1, 0);
