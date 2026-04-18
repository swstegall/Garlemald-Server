-- Ported from project-meteor-mirror/Data/sql/server_battlenpc_spawn_locations.sql
-- Table: server_battlenpc_spawn_locations

CREATE TABLE IF NOT EXISTS "server_battlenpc_spawn_locations" (
    "bnpcId" INTEGER PRIMARY KEY AUTOINCREMENT,
    "customDisplayName" TEXT NOT NULL DEFAULT '',
    "groupId" INTEGER NOT NULL,
    "positionX" REAL NOT NULL,
    "positionY" REAL NOT NULL,
    "positionZ" REAL NOT NULL,
    "rotation" REAL NOT NULL
);

INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (1, 'test', 1, 25.584, 200, -450, -2.514);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (2, 'test', 1, 20, 200, -444, -3.14);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (3, 'bloodthirsty_wolf', 2, 374.427, 4.4, -698.711, -1.942);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (4, 'bloodthirsty_wolf', 2, 375.377, 4.4, -700.247, -1.992);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (5, 'bloodthirsty_wolf', 2, 375.125, 4.4, -703.591, -1.54);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (6, 'yda', 3, 365.266, 4.122, -700.73, 1.5659);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (7, 'papalymo', 4, 365.89, 4.0943, -706.72, -0.718);
