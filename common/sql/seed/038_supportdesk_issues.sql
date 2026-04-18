-- Ported from project-meteor-mirror/Data/sql/supportdesk_issues.sql
-- Table: supportdesk_issues

CREATE TABLE IF NOT EXISTS "supportdesk_issues" (
    "slot" INTEGER NOT NULL,
    "title" TEXT NOT NULL,
    PRIMARY KEY ("slot")
);

INSERT OR IGNORE INTO "supportdesk_issues" ("slot", "title") VALUES
    ('0', 'Report Harassment');
INSERT OR IGNORE INTO "supportdesk_issues" ("slot", "title") VALUES
    ('1', 'Report Cheating');
INSERT OR IGNORE INTO "supportdesk_issues" ("slot", "title") VALUES
    ('2', 'Report a Bug or Glitch');
INSERT OR IGNORE INTO "supportdesk_issues" ("slot", "title") VALUES
    ('3', 'Leave Suggestion');
