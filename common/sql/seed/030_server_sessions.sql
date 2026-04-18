-- Ported from project-meteor-mirror/Data/sql/server_sessions.sql
-- Table: server_sessions

CREATE TABLE IF NOT EXISTS "server_sessions" (
    "id" TEXT NOT NULL,
    "characterId" INTEGER NOT NULL,
    "actorId" INTEGER NOT NULL,
    PRIMARY KEY ("id")
);
