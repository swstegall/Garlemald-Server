-- Ported from project-meteor-mirror/Data/sql/server_battlenpc_spell_list.sql
-- Table: server_battlenpc_spell_list

CREATE TABLE IF NOT EXISTS "server_battlenpc_spell_list" (
    "spellListId" INTEGER NOT NULL DEFAULT '0',
    "spellId" INTEGER NOT NULL DEFAULT '0',
    PRIMARY KEY ("spellListId", "spellId")
);
