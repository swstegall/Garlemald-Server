-- Ported from project-meteor-mirror/Data/sql/supportdesk_faqs.sql
-- Table: supportdesk_faqs

CREATE TABLE IF NOT EXISTS "supportdesk_faqs" (
    "slot" INTEGER NOT NULL,
    "languageCode" INTEGER NOT NULL,
    "title" TEXT NOT NULL,
    "body" TEXT NOT NULL,
    PRIMARY KEY ("slot", "languageCode")
);

INSERT OR IGNORE INTO "supportdesk_faqs" ("slot", "languageCode", "title", "body") VALUES
    ('0', '1', 'Welcome to FFXIV Classic', 'Welcome to the FFXIV 1.0 server emulator FFXIVClassic!

This is still currently a work in progress, and you may find bugs or issues as you play with this server. Keep in mind that this is not even remotely close to being finished, and that it is a work in progress.

Check out the blog at: 
http://ffxivclassic.fragmenterworks.com/ 
Check out videos at: 
https://www.youtube.com/channel/UCr2703_er1Dj7Lx5pzpQpfg');
