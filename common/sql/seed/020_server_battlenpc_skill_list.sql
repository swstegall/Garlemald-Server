-- Ported from project-meteor-mirror/Data/sql/server_battlenpc_skill_list.sql
-- Table: server_battlenpc_skill_list

CREATE TABLE IF NOT EXISTS "server_battlenpc_skill_list" (
    "skillListId" INTEGER NOT NULL DEFAULT '0',
    "skillId" INTEGER NOT NULL DEFAULT '0',
    PRIMARY KEY ("skillListId")
);
