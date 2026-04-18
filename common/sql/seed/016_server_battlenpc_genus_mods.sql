-- Ported from project-meteor-mirror/Data/sql/server_battlenpc_genus_mods.sql
-- Table: server_battlenpc_genus_mods

CREATE TABLE IF NOT EXISTS "server_battlenpc_genus_mods" (
    "genusId" INTEGER NOT NULL,
    "modId" INTEGER NOT NULL,
    "modVal" INTEGER NOT NULL,
    "isMobMod" INTEGER NOT NULL DEFAULT '0'
);
