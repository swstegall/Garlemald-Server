-- Ported from project-meteor-mirror/Data/sql/server_retainers.sql
-- Table: server_retainers

CREATE TABLE IF NOT EXISTS "server_retainers" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "actorClassId" INTEGER NOT NULL,
    "cdIDOffset" INTEGER NOT NULL DEFAULT '0',
    "placeName" INTEGER NOT NULL,
    "conditions" INTEGER NOT NULL DEFAULT '0',
    "level" INTEGER NOT NULL
);
