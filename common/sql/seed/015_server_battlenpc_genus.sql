-- Ported from project-meteor-mirror/Data/sql/server_battlenpc_genus.sql
-- Table: server_battlenpc_genus

CREATE TABLE IF NOT EXISTS "server_battlenpc_genus" (
    "genusId" INTEGER PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "modelSize" INTEGER NOT NULL DEFAULT '1',
    "speed" INTEGER NOT NULL DEFAULT '0',
    "kindredId" INTEGER NOT NULL DEFAULT '0',
    "kindredName" TEXT NOT NULL DEFAULT 'Unknown',
    "detection" INTEGER NOT NULL DEFAULT '0',
    "hpp" INTEGER NOT NULL DEFAULT '100',
    "mpp" INTEGER NOT NULL DEFAULT '100',
    "tpp" INTEGER NOT NULL DEFAULT '100',
    "str" INTEGER NOT NULL DEFAULT '1',
    "vit" INTEGER NOT NULL DEFAULT '1',
    "dex" INTEGER NOT NULL DEFAULT '1',
    "int" INTEGER NOT NULL DEFAULT '1',
    "mnd" INTEGER NOT NULL DEFAULT '1',
    "pie" INTEGER NOT NULL DEFAULT '1',
    "att" INTEGER NOT NULL DEFAULT '1',
    "acc" INTEGER NOT NULL DEFAULT '1',
    "def" INTEGER NOT NULL DEFAULT '1',
    "eva" INTEGER NOT NULL DEFAULT '1',
    "slash" REAL NOT NULL DEFAULT '1',
    "pierce" REAL NOT NULL DEFAULT '1',
    "h2h" REAL NOT NULL DEFAULT '1',
    "blunt" REAL NOT NULL DEFAULT '1',
    "fire" REAL NOT NULL DEFAULT '1',
    "ice" REAL NOT NULL DEFAULT '1',
    "wind" REAL NOT NULL DEFAULT '1',
    "lightning" REAL NOT NULL DEFAULT '1',
    "earth" REAL NOT NULL DEFAULT '1',
    "water" REAL NOT NULL DEFAULT '1',
    "element" INTEGER NOT NULL DEFAULT '0'
);

INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (1, 'Aldgoat', 1, 8, 1, 'Beast', 1, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (2, 'Antelope', 1, 8, 1, 'Beast', 1, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (3, 'Wolf', 1, 8, 1, 'Beast', 2, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (4, 'Opo-opo', 1, 8, 1, 'Beast', 1, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (5, 'Coeurl', 1, 8, 1, 'Beast', 15, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (6, 'Goobbue', 1, 8, 1, 'Beast', 4, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (7, 'Sheep', 1, 8, 1, 'Beast', 1, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (8, 'Buffalo', 1, 8, 1, 'Beast', 4, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (9, 'Boar', 1, 8, 1, 'Beast', 2, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (10, 'Moon-Mouse?', 1, 8, 1, 'Beast', 2, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (11, 'Mole', 1, 8, 1, 'Beast', 4, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (12, 'Rodent', 1, 8, 1, 'Beast', 2, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (13, 'Cactuar', 1, 8, 2, 'Plantoid', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (14, 'Funguar', 1, 8, 2, 'Plantoid', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (15, 'Flying-trap', 1, 8, 2, 'Plantoid', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (16, 'Morbol', 1, 8, 2, 'Plantoid', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (17, 'Orobon', 1, 8, 3, 'Aquan', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (18, 'Gigantoad', 1, 8, 3, 'Aquan', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (19, 'Salamander', 1, 8, 3, 'Aquan', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (20, 'Jelly-fish', 1, 8, 3, 'Aquan', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (21, 'Slug', 1, 8, 3, 'Aquan', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (22, 'Megalo-crab', 1, 8, 3, 'Aquan', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (23, 'Amaalja', 1, 8, 4, 'Spoken', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (24, 'Ixal', 1, 8, 4, 'Spoken', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (25, 'Qiqirn', 1, 8, 4, 'Spoken', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (26, 'Goblin', 1, 8, 4, 'Spoken', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (27, 'Kobold', 1, 8, 4, 'Spoken', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (28, 'Sylph', 1, 8, 4, 'Spoken', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (29, 'Person', 1, 8, 4, 'Spoken', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (30, 'Drake', 1, 8, 5, 'Reptilian', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (31, 'Basilisk', 1, 8, 5, 'Reptilian', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (32, 'Raptor', 1, 8, 5, 'Reptilian', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (33, 'Ant-ring', 1, 8, 6, 'Insect', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (34, 'Swarm', 1, 8, 6, 'Insect', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (35, 'Diremite', 1, 8, 6, 'Insect', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (36, 'Chigoe', 1, 8, 6, 'Insect', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (37, 'Gnat', 1, 8, 6, 'Insect', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (38, 'Beetle', 1, 8, 6, 'Insect', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (39, 'Yarzon', 1, 8, 6, 'Insect', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (40, 'Apkallu', 1, 8, 7, 'Avian', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (41, 'Vulture', 1, 8, 7, 'Avian', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (42, 'Dodo', 1, 8, 7, 'Avian', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (43, 'Bat', 1, 8, 7, 'Avian', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (44, 'Hippogryph', 1, 8, 7, 'Avian', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (45, 'Puk', 1, 8, 7, 'Avian', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (46, 'Ghost', 1, 8, 8, 'Undead', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (47, 'The-Damned', 1, 8, 8, 'Undead', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (48, 'Wight', 1, 8, 8, 'Undead', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (49, 'Coblyn', 1, 8, 9, 'Cursed', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (50, 'Spriggan', 1, 8, 9, 'Cursed', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (51, 'Ahriman', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (52, 'Imp', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (53, 'Will-O-Wisp', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (54, 'Fire-Elemental', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (55, 'Water-Elemental', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (56, 'Earth-Elemental', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (57, 'Lightning-Elemental', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (58, 'Ice-Elemental', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (59, 'Wind-Elemental', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (60, 'Ogre', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (61, 'Phurble', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (62, 'Plasmoid', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (63, 'Flan', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (64, 'Bomb', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
INSERT OR IGNORE INTO "server_battlenpc_genus" ("genusId", "name", "modelSize", "speed", "kindredId", "kindredName", "detection", "hpp", "mpp", "tpp", "str", "vit", "dex", "int", "mnd", "pie", "att", "acc", "def", "eva", "slash", "pierce", "h2h", "blunt", "fire", "ice", "wind", "lightning", "earth", "water", "element") VALUES
    (65, 'Grenade', 1, 8, 10, 'Voidsent', 0, 100, 100, 100, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0);
