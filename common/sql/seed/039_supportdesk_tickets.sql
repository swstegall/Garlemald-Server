-- Ported from project-meteor-mirror/Data/sql/supportdesk_tickets.sql
-- Table: supportdesk_tickets

CREATE TABLE IF NOT EXISTS "supportdesk_tickets" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "title" TEXT NOT NULL,
    "body" TEXT NOT NULL,
    "langCode" INTEGER NOT NULL,
    "isOpen" INTEGER NOT NULL DEFAULT '1'
);
