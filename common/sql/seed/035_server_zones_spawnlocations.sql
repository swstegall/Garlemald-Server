-- Ported from project-meteor-mirror/Data/sql/server_zones_spawnlocations.sql
-- Table: server_zones_spawnlocations

CREATE TABLE IF NOT EXISTS "server_zones_spawnlocations" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "zoneId" INTEGER NOT NULL,
    "privateAreaName" TEXT DEFAULT NULL,
    "spawnType" INTEGER DEFAULT '0',
    "spawnX" REAL NOT NULL,
    "spawnY" REAL NOT NULL,
    "spawnZ" REAL NOT NULL,
    "spawnRotation" REAL NOT NULL
);

INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('1', '155', null, '15', '58.92', '4', '-1219.07', '0.52');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('2', '133', null, '15', '-444.266', '39.518', '191', '1.9');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('3', '175', null, '15', '-110.157', '202', '171.345', '0');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('4', '193', null, '15', '0.016', '10.35', '-36.91', '0.025');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('5', '166', null, '15', '356.09', '3.74', '-701.62', '-1.4');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('6', '184', null, '15', '5.36433', '196', '133.656', '-2.84938');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('7', '128', null, '15', '-8.48', '45.36', '139.5', '2.02');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('8', '230', 'PrivateAreaMasterPast', '15', '-838.1', '6', '231.94', '1.1');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('9', '193', null, '16', '-5', '16.35', '6', '0.5');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('10', '166', null, '16', '356.09', '3.74', '-701.62', '-1.4');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('11', '244', null, '15', '0.048', '0', '-5.737', '0');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('12', '244', null, '15', '-160.048', '0', '-165.737', '0');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('13', '244', null, '15', '160.048', '0', '154.263', '0');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('14', '150', null, '15', '333.271', '5.889', '-943.275', '0.794');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('15', '133', null, '15', '-8.062', '45.429', '139.364', '2.955');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('16', '170', null, '15', '-27.015', '181.798', '-79.72', '2.513');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('17', '184', null, '16', '-24.34', '192', '34.22', '0.78');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('18', '184', null, '15', '-24.34', '192', '34.22', '0.78');
INSERT OR IGNORE INTO "server_zones_spawnlocations" ("id", "zoneId", "privateAreaName", "spawnType", "spawnX", "spawnY", "spawnZ", "spawnRotation") VALUES
    ('19', '184', null, '15', '-22', '196', '87', '1.8');
