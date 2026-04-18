-- Ported from project-meteor-mirror/Data/sql/servers.sql
-- Table: servers

CREATE TABLE IF NOT EXISTS "servers" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "address" TEXT NOT NULL,
    "port" INTEGER NOT NULL,
    "listPosition" INTEGER NOT NULL,
    "numchars" INTEGER NOT NULL DEFAULT '0',
    "maxchars" INTEGER NOT NULL DEFAULT '5000',
    "isActive" INTEGER NOT NULL
);

INSERT OR IGNORE INTO "servers" ("id", "name", "address", "port", "listPosition", "numchars", "maxchars", "isActive") VALUES
    (1, 'Fernehalwes', '127.0.0.1', 54992, 1, 1, 5000, 1);
