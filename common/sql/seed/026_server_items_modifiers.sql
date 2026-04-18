-- Ported from project-meteor-mirror/Data/sql/server_items_modifiers.sql
-- Table: server_items_modifiers

CREATE TABLE IF NOT EXISTS "server_items_modifiers" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "durability" INTEGER NOT NULL DEFAULT '0',
    "mainQuality" INTEGER NOT NULL DEFAULT '0',
    "subQuality1" INTEGER NOT NULL DEFAULT '0',
    "subQuality2" INTEGER NOT NULL DEFAULT '0',
    "subQuality3" INTEGER NOT NULL DEFAULT '0',
    "param1" INTEGER NOT NULL DEFAULT '0',
    "param2" INTEGER NOT NULL DEFAULT '0',
    "param3" INTEGER NOT NULL DEFAULT '0',
    "spiritbind" INTEGER NOT NULL DEFAULT '0',
    "materia1" INTEGER NOT NULL DEFAULT '0',
    "materia2" INTEGER NOT NULL DEFAULT '0',
    "materia3" INTEGER NOT NULL DEFAULT '0',
    "materia4" INTEGER NOT NULL DEFAULT '0',
    "materia5" INTEGER NOT NULL DEFAULT '0'
);
