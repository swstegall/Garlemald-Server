-- Ported from project-meteor-mirror/Data/sql/server_items_dealing.sql
-- Table: server_items_dealing

CREATE TABLE IF NOT EXISTS "server_items_dealing" (
    "id" INTEGER NOT NULL,
    "dealingValue" INTEGER NOT NULL DEFAULT '0',
    "dealingMode" INTEGER NOT NULL DEFAULT '0',
    "dealingAttached1" INTEGER DEFAULT '0',
    "dealingAttached2" INTEGER NOT NULL DEFAULT '0',
    "dealingAttached3" INTEGER NOT NULL DEFAULT '0',
    "dealingTag" INTEGER NOT NULL DEFAULT '0',
    "bazaarMode" INTEGER NOT NULL DEFAULT '0',
    PRIMARY KEY ("id")
);
