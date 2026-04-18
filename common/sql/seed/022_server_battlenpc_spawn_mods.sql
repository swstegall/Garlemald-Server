-- Ported from project-meteor-mirror/Data/sql/server_battlenpc_spawn_mods.sql
-- Table: server_battlenpc_spawn_mods

CREATE TABLE IF NOT EXISTS "server_battlenpc_spawn_mods" (
    "bnpcId" INTEGER NOT NULL,
    "modId" INTEGER NOT NULL,
    "modVal" INTEGER NOT NULL,
    "isMobMod" INTEGER NOT NULL DEFAULT '0'
);

INSERT OR IGNORE INTO "server_battlenpc_spawn_mods" ("bnpcId", "modId", "modVal", "isMobMod") VALUES
    (3, 25, 30, 1);
INSERT OR IGNORE INTO "server_battlenpc_spawn_mods" ("bnpcId", "modId", "modVal", "isMobMod") VALUES
    (4, 25, 35, 1);
INSERT OR IGNORE INTO "server_battlenpc_spawn_mods" ("bnpcId", "modId", "modVal", "isMobMod") VALUES
    (5, 25, 40, 1);
