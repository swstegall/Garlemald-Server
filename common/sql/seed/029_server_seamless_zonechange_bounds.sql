-- Ported from project-meteor-mirror/Data/sql/server_seamless_zonechange_bounds.sql
-- Table: server_seamless_zonechange_bounds

CREATE TABLE IF NOT EXISTS "server_seamless_zonechange_bounds" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "regionId" INTEGER NOT NULL,
    "zoneId1" INTEGER NOT NULL,
    "zoneId2" INTEGER NOT NULL,
    "zone1_boundingbox_x1" REAL NOT NULL,
    "zone1_boundingbox_y1" REAL NOT NULL,
    "zone1_boundingbox_x2" REAL NOT NULL,
    "zone1_boundingbox_y2" REAL NOT NULL,
    "zone2_boundingbox_x1" REAL NOT NULL,
    "zone2_boundingbox_x2" REAL NOT NULL,
    "zone2_boundingbox_y1" REAL NOT NULL,
    "zone2_boundingbox_y2" REAL NOT NULL,
    "merge_boundingbox_x1" REAL NOT NULL,
    "merge_boundingbox_y1" REAL NOT NULL,
    "merge_boundingbox_x2" REAL NOT NULL,
    "merge_boundingbox_y2" REAL NOT NULL
);

INSERT OR IGNORE INTO "server_seamless_zonechange_bounds" ("id", "regionId", "zoneId1", "zoneId2", "zone1_boundingbox_x1", "zone1_boundingbox_y1", "zone1_boundingbox_x2", "zone1_boundingbox_y2", "zone2_boundingbox_x1", "zone2_boundingbox_x2", "zone2_boundingbox_y1", "zone2_boundingbox_y2", "merge_boundingbox_x1", "merge_boundingbox_y1", "merge_boundingbox_x2", "merge_boundingbox_y2") VALUES
    ('1', '103', '155', '206', '115', '-1219', '55', '-1217', '33', '95', '-1279', '-1261', '55', '-1219', '95', '-1261');
INSERT OR IGNORE INTO "server_seamless_zonechange_bounds" ("id", "regionId", "zoneId1", "zoneId2", "zone1_boundingbox_x1", "zone1_boundingbox_y1", "zone1_boundingbox_x2", "zone1_boundingbox_y2", "zone2_boundingbox_x1", "zone2_boundingbox_x2", "zone2_boundingbox_y1", "zone2_boundingbox_y2", "merge_boundingbox_x1", "merge_boundingbox_y1", "merge_boundingbox_x2", "merge_boundingbox_y2") VALUES
    ('2', '103', '155', '150', '255', '-1139', '304', '-1125', '304', '338', '-1066', '-1046', '255', '-1125', '338', '-1066');
INSERT OR IGNORE INTO "server_seamless_zonechange_bounds" ("id", "regionId", "zoneId1", "zoneId2", "zone1_boundingbox_x1", "zone1_boundingbox_y1", "zone1_boundingbox_x2", "zone1_boundingbox_y2", "zone2_boundingbox_x1", "zone2_boundingbox_x2", "zone2_boundingbox_y1", "zone2_boundingbox_y2", "merge_boundingbox_x1", "merge_boundingbox_y1", "merge_boundingbox_x2", "merge_boundingbox_y2") VALUES
    ('3', '101', '133', '230', '-457', '131', '-436', '142', '-460', '-439', '92', '100', '-454', '101', '-439', '128');
INSERT OR IGNORE INTO "server_seamless_zonechange_bounds" ("id", "regionId", "zoneId1", "zoneId2", "zone1_boundingbox_x1", "zone1_boundingbox_y1", "zone1_boundingbox_x2", "zone1_boundingbox_y2", "zone2_boundingbox_x1", "zone2_boundingbox_x2", "zone2_boundingbox_y1", "zone2_boundingbox_y2", "merge_boundingbox_x1", "merge_boundingbox_y1", "merge_boundingbox_x2", "merge_boundingbox_y2") VALUES
    ('4', '101', '133', '230', '-486', '228', '-501', '218', '-482', '-503', '255', '242', '-490', '238', '-501', '229');
INSERT OR IGNORE INTO "server_seamless_zonechange_bounds" ("id", "regionId", "zoneId1", "zoneId2", "zone1_boundingbox_x1", "zone1_boundingbox_y1", "zone1_boundingbox_x2", "zone1_boundingbox_y2", "zone2_boundingbox_x1", "zone2_boundingbox_x2", "zone2_boundingbox_y1", "zone2_boundingbox_y2", "merge_boundingbox_x1", "merge_boundingbox_y1", "merge_boundingbox_x2", "merge_boundingbox_y2") VALUES
    ('5', '101', '133', '128', '-85', '165', '-79', '185', '-51', '-47', '149', '167', '-71', '160', '-69', '174');
INSERT OR IGNORE INTO "server_seamless_zonechange_bounds" ("id", "regionId", "zoneId1", "zoneId2", "zone1_boundingbox_x1", "zone1_boundingbox_y1", "zone1_boundingbox_x2", "zone1_boundingbox_y2", "zone2_boundingbox_x1", "zone2_boundingbox_x2", "zone2_boundingbox_y1", "zone2_boundingbox_y2", "merge_boundingbox_x1", "merge_boundingbox_y1", "merge_boundingbox_x2", "merge_boundingbox_y2") VALUES
    ('6', '101', '133', '230', '-483', '200', '-496', '181', '-506', '-514', '206', '177', '-500', '198', '-505', '185');
INSERT OR IGNORE INTO "server_seamless_zonechange_bounds" ("id", "regionId", "zoneId1", "zoneId2", "zone1_boundingbox_x1", "zone1_boundingbox_y1", "zone1_boundingbox_x2", "zone1_boundingbox_y2", "zone2_boundingbox_x1", "zone2_boundingbox_x2", "zone2_boundingbox_y1", "zone2_boundingbox_y2", "merge_boundingbox_x1", "merge_boundingbox_y1", "merge_boundingbox_x2", "merge_boundingbox_y2") VALUES
    ('7', '104', '170', '209', '87', '178', '110', '189', '89', '108', '142', '150', '94', '158', '108', '167');
INSERT OR IGNORE INTO "server_seamless_zonechange_bounds" ("id", "regionId", "zoneId1", "zoneId2", "zone1_boundingbox_x1", "zone1_boundingbox_y1", "zone1_boundingbox_x2", "zone1_boundingbox_y2", "zone2_boundingbox_x1", "zone2_boundingbox_x2", "zone2_boundingbox_y1", "zone2_boundingbox_y2", "merge_boundingbox_x1", "merge_boundingbox_y1", "merge_boundingbox_x2", "merge_boundingbox_y2") VALUES
    ('8', '104', '175', '209', '-134', '84', '-95', '92', '-120', '-82', '139', '143', '-120', '125', '-96', '124');
INSERT OR IGNORE INTO "server_seamless_zonechange_bounds" ("id", "regionId", "zoneId1", "zoneId2", "zone1_boundingbox_x1", "zone1_boundingbox_y1", "zone1_boundingbox_x2", "zone1_boundingbox_y2", "zone2_boundingbox_x1", "zone2_boundingbox_x2", "zone2_boundingbox_y1", "zone2_boundingbox_y2", "merge_boundingbox_x1", "merge_boundingbox_y1", "merge_boundingbox_x2", "merge_boundingbox_y2") VALUES
    ('9', '104', '170', '175', '-70', '-47', '-47', '-17', '-117', '-108', '-43', '-28', '-99', '-43', '-86', '-28');
INSERT OR IGNORE INTO "server_seamless_zonechange_bounds" ("id", "regionId", "zoneId1", "zoneId2", "zone1_boundingbox_x1", "zone1_boundingbox_y1", "zone1_boundingbox_x2", "zone1_boundingbox_y2", "zone2_boundingbox_x1", "zone2_boundingbox_x2", "zone2_boundingbox_y1", "zone2_boundingbox_y2", "merge_boundingbox_x1", "merge_boundingbox_y1", "merge_boundingbox_x2", "merge_boundingbox_y2") VALUES
    ('10', '104', '170', '175', '-39', '-33', '-24', '-9', '22', '23', '-7', '22', '-7', '-26', '-1', '-4');
INSERT OR IGNORE INTO "server_seamless_zonechange_bounds" ("id", "regionId", "zoneId1", "zoneId2", "zone1_boundingbox_x1", "zone1_boundingbox_y1", "zone1_boundingbox_x2", "zone1_boundingbox_y2", "zone2_boundingbox_x1", "zone2_boundingbox_x2", "zone2_boundingbox_y1", "zone2_boundingbox_y2", "merge_boundingbox_x1", "merge_boundingbox_y1", "merge_boundingbox_x2", "merge_boundingbox_y2") VALUES
    ('11', '104', '175', '209', '-243', '82', '-208', '107', '-264', '-230', '138', '173', '-254', '109', '-220', '128');
INSERT OR IGNORE INTO "server_seamless_zonechange_bounds" ("id", "regionId", "zoneId1", "zoneId2", "zone1_boundingbox_x1", "zone1_boundingbox_y1", "zone1_boundingbox_x2", "zone1_boundingbox_y2", "zone2_boundingbox_x1", "zone2_boundingbox_x2", "zone2_boundingbox_y1", "zone2_boundingbox_y2", "merge_boundingbox_x1", "merge_boundingbox_y1", "merge_boundingbox_x2", "merge_boundingbox_y2") VALUES
    ('12', '104', '175', '209', '0', '173', '24', '179', '-23', '9', '204', '232', '-6', '185', '13', '201');
INSERT OR IGNORE INTO "server_seamless_zonechange_bounds" ("id", "regionId", "zoneId1", "zoneId2", "zone1_boundingbox_x1", "zone1_boundingbox_y1", "zone1_boundingbox_x2", "zone1_boundingbox_y2", "zone2_boundingbox_x1", "zone2_boundingbox_x2", "zone2_boundingbox_y1", "zone2_boundingbox_y2", "merge_boundingbox_x1", "merge_boundingbox_y1", "merge_boundingbox_x2", "merge_boundingbox_y2") VALUES
    ('13', '104', '175', '209', '-20', '99', '5', '119', '-57', '-31', '124', '145', '-41', '115', '-15', '127');
