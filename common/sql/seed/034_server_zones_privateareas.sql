-- Ported from project-meteor-mirror/Data/sql/server_zones_privateareas.sql
-- Table: server_zones_privateareas

CREATE TABLE IF NOT EXISTS "server_zones_privateareas" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "parentZoneId" INTEGER NOT NULL,
    "className" TEXT NOT NULL,
    "privateAreaName" TEXT NOT NULL,
    "privateAreaType" INTEGER NOT NULL,
    "dayMusic" INTEGER DEFAULT '0',
    "nightMusic" INTEGER DEFAULT '0',
    "battleMusic" INTEGER DEFAULT '0'
);

INSERT OR IGNORE INTO "server_zones_privateareas" ("id", "parentZoneId", "className", "privateAreaName", "privateAreaType", "dayMusic", "nightMusic", "battleMusic") VALUES
    ('1', '184', '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', '1', '66', '0', '0');
INSERT OR IGNORE INTO "server_zones_privateareas" ("id", "parentZoneId", "className", "privateAreaName", "privateAreaType", "dayMusic", "nightMusic", "battleMusic") VALUES
    ('2', '230', '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', '1', '59', '0', '0');
INSERT OR IGNORE INTO "server_zones_privateareas" ("id", "parentZoneId", "className", "privateAreaName", "privateAreaType", "dayMusic", "nightMusic", "battleMusic") VALUES
    ('4', '133', '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', '2', '40', '0', '0');
INSERT OR IGNORE INTO "server_zones_privateareas" ("id", "parentZoneId", "className", "privateAreaName", "privateAreaType", "dayMusic", "nightMusic", "battleMusic") VALUES
    ('5', '155', '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', '1', '51', '0', '0');
INSERT OR IGNORE INTO "server_zones_privateareas" ("id", "parentZoneId", "className", "privateAreaName", "privateAreaType", "dayMusic", "nightMusic", "battleMusic") VALUES
    ('6', '155', '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', '2', '40', '0', '0');
INSERT OR IGNORE INTO "server_zones_privateareas" ("id", "parentZoneId", "className", "privateAreaName", "privateAreaType", "dayMusic", "nightMusic", "battleMusic") VALUES
    ('8', '175', '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', '3', '66', '0', '0');
INSERT OR IGNORE INTO "server_zones_privateareas" ("id", "parentZoneId", "className", "privateAreaName", "privateAreaType", "dayMusic", "nightMusic", "battleMusic") VALUES
    ('9', '175', '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', '4', '40', '0', '0');
INSERT OR IGNORE INTO "server_zones_privateareas" ("id", "parentZoneId", "className", "privateAreaName", "privateAreaType", "dayMusic", "nightMusic", "battleMusic") VALUES
    ('10', '180', '/Area/PrivateArea/PrivateAreaMasterBranch', 'PrivateAreaMasterMarket', '102', '48', '48', '48');
