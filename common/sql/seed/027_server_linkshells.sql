-- Ported from project-meteor-mirror/Data/sql/server_linkshells.sql
-- Table: server_linkshells

CREATE TABLE IF NOT EXISTS "server_linkshells" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "crestIcon" INTEGER NOT NULL,
    "master" INTEGER NOT NULL DEFAULT '0',
    "rank" INTEGER NOT NULL
);
