-- Ported from project-meteor-mirror/Data/sql/server_zones.sql
-- Table: server_zones

CREATE TABLE IF NOT EXISTS "server_zones" (
    "id" INTEGER NOT NULL,
    "regionId" INTEGER NOT NULL,
    "zoneName" TEXT DEFAULT NULL,
    "placeName" TEXT NOT NULL,
    "serverIp" TEXT NOT NULL,
    "serverPort" INTEGER NOT NULL,
    "classPath" TEXT NOT NULL,
    "dayMusic" INTEGER DEFAULT '0',
    "nightMusic" INTEGER DEFAULT '0',
    "battleMusic" INTEGER DEFAULT '0',
    "isIsolated" INTEGER DEFAULT '0',
    "isInn" INTEGER DEFAULT '0',
    "canRideChocobo" INTEGER DEFAULT '1',
    "canStealth" INTEGER DEFAULT '0',
    "isInstanceRaid" INTEGER DEFAULT '0',
    "loadNavMesh" INTEGER NOT NULL,
    PRIMARY KEY ("id")
);

INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (0, 0, NULL, '--', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (128, 101, 'sea0Field01', 'Lower La Noscea', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterSeaS0', 60, 60, 21, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (129, 101, 'sea0Field02', 'Western La Noscea', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterSeaS0', 60, 60, 21, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (130, 101, 'sea0Field03', 'Eastern La Noscea', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterSeaS0', 60, 60, 21, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (131, 101, 'sea0Dungeon01', 'Mistbeard Cove', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterSeaS0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (132, 101, 'sea0Dungeon03', 'Cassiopeia Hollow', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterSeaS0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (133, 101, 'sea0Town01', 'Limsa Lominsa', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterSeaS0', 59, 59, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (134, 202, 'sea0Market01', 'Market Wards', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterMarketSeaS0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (135, 101, 'sea0Field04', 'Upper La Noscea', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterSeaS0', 60, 60, 21, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (137, 101, 'sea0Dungeon06', 'U''Ghamaro Mines', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterSeaS0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (138, 101, NULL, 'La Noscea', '127.0.0.1', 1989, '', 60, 60, 21, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (139, 112, 'sea0Field01a', 'The Cieldalaes', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterSeaS0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (140, 101, NULL, 'Sailors Ward', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (141, 101, 'sea0Field01a', 'Lower La Noscea', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterSeaS0', 60, 60, 21, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (143, 102, 'roc0Field01', 'Coerthas Central Highlands', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR0', 55, 55, 15, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (144, 102, 'roc0Field02', 'Coerthas Eastern Highlands', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR0', 55, 55, 15, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (145, 102, 'roc0Field03', 'Coerthas Eastern Lowlands', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR0', 55, 55, 15, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (146, 102, NULL, 'Coerthas', '127.0.0.1', 1989, '', 55, 55, 15, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (147, 102, 'roc0Field04', 'Coerthas Central Lowlands', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR0', 55, 55, 15, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (148, 102, 'roc0Field05', 'Coerthas Western Highlands', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR0', 55, 55, 15, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (150, 103, 'fst0Field01', 'Central Shroud', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterFstF0', 52, 52, 13, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (151, 103, 'fst0Field02', 'East Shroud', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterFstF0', 52, 52, 13, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (152, 103, 'fst0Field03', 'North Shroud', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterFstF0', 52, 52, 13, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (153, 103, 'fst0Field04', 'West Shroud', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterFstF0', 52, 52, 13, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (154, 103, 'fst0Field05', 'South Shroud', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterFstF0', 52, 52, 13, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (155, 103, 'fst0Town01', 'Gridania', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterFstF0', 51, 51, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (156, 103, NULL, 'The Black Shroud', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (157, 103, 'fst0Dungeon01', 'The Mun-Tuy Cellars', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterFstF0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (158, 103, 'fst0Dungeon02', 'The Tam-Tara Deepcroft', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterFstF0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (159, 103, 'fst0Dungeon03', 'The Thousand Maws of Toto-Rak', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterFstF0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (160, 204, 'fst0Market01', 'Market Wards', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterMarketFstF0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (161, 103, NULL, 'Peasants Ward', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (162, 103, 'fst0Field01a', 'Central Shroud', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterFstF0', 52, 52, 13, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (164, 106, 'fst0Battle01', 'Central Shroud', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleFstF0', 0, 0, 13, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (165, 106, 'fst0Battle02', 'Central Shroud', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleFstF0', 0, 0, 13, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (166, 106, 'fst0Battle03', 'Central Shroud', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleFstF0', 0, 0, 13, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (167, 106, 'fst0Battle04', 'Central Shroud', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleFstF0', 0, 0, 13, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (168, 106, 'fst0Battle05', 'Central Shroud', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleFstF0', 0, 0, 13, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (170, 104, 'wil0Field01', 'Central Thanalan', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterWilW0', 68, 68, 25, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (171, 104, 'wil0Field02', 'Eastern Thanalan', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterWilW0', 68, 68, 25, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (172, 104, 'wil0Field03', 'Western Thanalan', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterWilW0', 68, 68, 25, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (173, 104, 'wil0Field04', 'Northern Thanalan', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterWilW0', 68, 68, 25, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (174, 104, 'wil0Field05', 'Southern Thanalan', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterWilW0', 68, 68, 25, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (175, 104, 'wil0Town01', 'Ul''dah', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterWilW0', 66, 66, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (176, 104, 'wil0Dungeon02', 'Nanawa Mines', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterWilW0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (177, 207, '_jail', '-', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterJail', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (178, 104, 'wil0Dungeon04', 'Copperbell Mines', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterWilW0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (179, 104, NULL, 'Thanalan', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (180, 205, 'wil0Market01', 'Market Wards', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterMarketWilW0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (181, 104, NULL, 'Merchants Ward', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (182, 104, NULL, 'Central Thanalan', '127.0.0.1', 1989, '', 68, 68, 25, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (184, 107, 'wil0Battle01', 'Ul''dah', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleWilW0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (185, 107, 'wil0Battle01', 'Ul''dah', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleWilW0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (186, 104, 'wil0Battle02', 'Ul''dah', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleWilW0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (187, 104, 'wil0Battle03', 'Ul''dah', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleWilW0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (188, 104, 'wil0Battle04', 'Ul''dah', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleWilW0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (190, 105, 'lak0Field01', 'Mor Dhona', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterLakL0', 49, 49, 11, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (192, 112, 'ocn1Battle01', 'Rhotano Sea', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleOcnO1', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (193, 111, 'ocn0Battle02', 'Rhotano Sea', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleOcnO0', 7, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (194, 112, 'ocn1Battle03', 'Rhotano Sea', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleOcnO1', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (195, 112, 'ocn1Battle04', 'Rhotano Sea', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleOcnO1', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (196, 112, 'ocn1Battle05', 'Rhotano Sea', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleOcnO1', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (198, 112, 'ocn1Battle06', 'Rhotano Sea', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterBattleOcnO1', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (200, 805, 'sea1Cruise01', 'Strait of Merlthor', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterCruiseSeaS1', 65, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (201, 208, 'prv0Cottage00', '-', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterCottagePrv00', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (204, 101, 'sea0Field02a', 'Western La Noscea', '127.0.0.1', 1989, '', 60, 60, 21, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (205, 101, 'sea0Field03a', 'Eastern La Noscea', '127.0.0.1', 1989, '', 60, 60, 21, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (206, 103, 'fst0Town01a', 'Gridania', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterFstF0', 51, 51, 13, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (207, 103, 'fst0Field03a', 'North Shroud', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterFstF0', 52, 52, 13, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (208, 103, 'fst0Field05a', 'South Shroud', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterFstF0', 52, 52, 13, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (209, 104, 'wil0Town01a', 'Ul''dah', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterWilW0', 66, 66, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (210, 104, NULL, 'Eastern Thanalan', '127.0.0.1', 1989, '', 68, 68, 25, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (211, 104, NULL, 'Western Thanalan', '127.0.0.1', 1989, '', 68, 68, 25, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (230, 101, 'sea0Town01a', 'Limsa Lominsa', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterSeaS0', 59, 59, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (231, 102, 'roc0Dungeon01', 'Dzemael Darkhold', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (232, 202, 'sea0Office01', 'Maelstrom Command', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterOfficeSeaS0', 3, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (233, 205, 'wil0Office01', 'Hall of Flames', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterOfficeWilW0', 4, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (234, 204, 'fst0Office01', 'Adders'' Nest', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterOfficeFstF0', 2, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (235, 101, NULL, 'Shposhae', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (236, 101, 'sea1Field01', 'Locke''s Lie', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterSeaS1', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (237, 101, NULL, 'Turtleback Island', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (238, 103, 'fst0Field04', 'Thornmarch', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (239, 102, 'roc0Field02a', 'The Howling Eye', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (240, 104, 'wil0Field05a', 'The Bowl of Embers', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (244, 209, 'prv0Inn01', 'Inn Room', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterPrvI0', 61, 61, 0, 0, 1, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (245, 102, 'roc0Dungeon04', 'The Aurum Vale', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (246, 104, NULL, 'Cutter''s Cry', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (247, 103, NULL, 'North Shroud', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (248, 101, NULL, 'Western La Noscea', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (249, 104, NULL, 'Eastern Thanalan', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (250, 102, 'roc0Field02a', 'The Howling Eye', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (251, 105, NULL, 'Transmission Tower', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (252, 102, 'roc0Dungeon04', 'The Aurum Vale', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (253, 102, 'roc0Dungeon04', 'The Aurum Vale', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (254, 104, NULL, 'Cutter''s Cry', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (255, 104, NULL, 'Cutter''s Cry', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (256, 102, 'roc0Field02a', 'The Howling Eye', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR0', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (257, 109, 'roc1Field01', 'Rivenroad', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR1', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (258, 103, NULL, 'North Shroud', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (259, 103, NULL, 'North Shroud', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (260, 101, NULL, 'Western La Noscea', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (261, 101, NULL, 'Western La Noscea', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (262, 104, NULL, 'Eastern Thanalan', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (263, 104, NULL, 'Eastern Thanalan', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (264, 105, 'lak0Field01', 'Transmission Tower', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 1, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (265, 104, NULL, 'The Bowl of Embers', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (266, 105, 'lak0Field01a', 'Mor Dhona', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterLakL0', 49, 49, 11, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (267, 109, 'roc1Field02', 'Rivenroad', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR1', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (268, 109, 'roc1Field03', 'Rivenroad', '127.0.0.1', 1989, '/Area/Zone/ZoneMasterRocR1', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (269, 101, NULL, 'Locke''s Lie', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones" ("id", "regionId", "zoneName", "placeName", "serverIp", "serverPort", "classPath", "dayMusic", "nightMusic", "battleMusic", "isIsolated", "isInn", "canRideChocobo", "canStealth", "isInstanceRaid", "loadNavMesh") VALUES
    (270, 101, NULL, 'Turtleback Island', '127.0.0.1', 1989, '', 0, 0, 0, 0, 0, 0, 0, 0, 0);
