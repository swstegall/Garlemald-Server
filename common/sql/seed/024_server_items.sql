-- Ported from project-meteor-mirror/Data/sql/server_items.sql
-- Table: server_items

CREATE TABLE IF NOT EXISTS "server_items" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "itemId" INTEGER NOT NULL,
    "quantity" INTEGER NOT NULL DEFAULT '0',
    "quality" INTEGER NOT NULL DEFAULT '0'
);
