-- Ported from project-meteor-mirror/Data/sql/gamedata_items_graphics_extra.sql
-- Table: gamedata_items_graphics_extra

CREATE TABLE IF NOT EXISTS "gamedata_items_graphics_extra" (
    "catalogID" INTEGER NOT NULL,
    "offHandWeaponId" INTEGER NOT NULL DEFAULT '0',
    "offHandEquipmentId" INTEGER NOT NULL DEFAULT '0',
    "offHandVarientId" INTEGER NOT NULL DEFAULT '0',
    PRIMARY KEY ("catalogID")
);

INSERT OR IGNORE INTO "gamedata_items_graphics_extra" ("catalogID", "offHandWeaponId", "offHandEquipmentId", "offHandVarientId") VALUES
    ('4020001', '58', '1', '0');
INSERT OR IGNORE INTO "gamedata_items_graphics_extra" ("catalogID", "offHandWeaponId", "offHandEquipmentId", "offHandVarientId") VALUES
    ('4070001', '226', '1', '0');
