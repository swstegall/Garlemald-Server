-- Ported from project-meteor-mirror/Data/sql/server_battle_traits.sql
-- Table: server_battle_traits

CREATE TABLE IF NOT EXISTS "server_battle_traits" (
    "id" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    "classJob" INTEGER NOT NULL,
    "lvl" INTEGER NOT NULL,
    "modifier" INTEGER NOT NULL DEFAULT '0',
    "bonus" INTEGER NOT NULL DEFAULT '0',
    PRIMARY KEY ("id")
);

INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27240, 'enhanced_hawks_eye', 7, 28, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27242, 'enhanced_barrage', 7, 44, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27241, 'enhanced_quelling_strike', 7, 32, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27243, 'enhanced_raging_strike', 7, 36, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27244, 'enhanced_decoy', 7, 16, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27245, 'swift_chameleon', 7, 48, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27246, 'enhanced_physical_crit_accuracy', 7, 40, 19, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27247, 'enhanced_physical_crit_evasion', 7, 20, 20, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27248, 'enhanced_physical_evasion', 7, 12, 16, 8);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27249, 'enhanced_physical_accuracy', 7, 8, 15, 8);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27250, 'enhanced_physical_accuracy_ii', 7, 24, 15, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27120, 'enhanced_second_wind', 2, 20, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27121, 'enhanced_blindside', 2, 24, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27122, 'swift_taunt', 2, 48, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27123, 'enhanced_featherfoot', 2, 28, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27124, 'enhanced_fists_of_fire', 2, 44, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27125, 'enhanced_fists_of_earth', 2, 36, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27126, 'enhanced_physical_accuracy', 2, 16, 15, 8);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27127, 'enhanced_physical_attack', 2, 8, 17, 8);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27128, 'enhanced_physical_attack_ii', 2, 40, 17, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27129, 'enhanced_evasion', 2, 12, 16, 8);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27130, 'enhanced_physical_crit_damage', 2, 32, 21, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27160, 'enhanced_sentinel', 3, 36, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27161, 'enhanced_flash', 3, 28, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27162, 'enhanced_flash_ii', 3, 48, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27163, 'enhanced_rampart', 3, 12, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27164, 'swift_aegis_boon', 3, 20, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27165, 'enhanced_outmaneuver', 3, 44, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27167, 'enhanced_block_rate', 3, 16, 41, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27166, 'enhanced_physical_crit_resilience', 3, 32, 22, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27168, 'enhanced_physical_defense', 3, 8, 18, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27169, 'enhanced_physical_defense_ii', 3, 24, 18, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27170, 'enhanced_physical_defense_iii', 3, 40, 18, 12);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27200, 'enhanced_provoke', 4, 28, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27201, 'swift_foresight', 4, 20, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27202, 'swift_bloodbath', 4, 16, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27203, 'enhanced_enduring_march', 4, 48, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27204, 'enhanced_rampage', 4, 44, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27205, 'enhanced_berserk', 4, 36, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27206, 'enhanced_physical_crit_evasion', 4, 32, 20, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27207, 'enhanced_parry', 4, 24, 39, 8);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27208, 'enhanced_physical_defense', 4, 12, 18, 8);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27209, 'enhanced_physical_defense_ii', 4, 40, 18, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27210, 'enhanced_physical_attack_power', 4, 8, 17, 8);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27280, 'enhanced_invigorate', 8, 28, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27281, 'enhanced_power_surge', 8, 44, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27282, 'enhanced_life_surge', 8, 32, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27283, 'enhanced_blood_for_blood', 8, 48, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27284, 'swift_blood_for_blood', 8, 16, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27285, 'enhanced_keen_flurry', 8, 36, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27286, 'store_tp', 8, 12, 50, 50);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27287, 'enhanced_physical_crit_accuracy', 8, 24, 19, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27288, 'enhanced_physical_attack_power', 8, 8, 17, 8);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27289, 'enhanced_physical_attack_power_ii', 8, 20, 17, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27290, 'enhanced_physical_attack_power_iii', 8, 40, 17, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27320, 'swift_dark_seal', 22, 36, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27321, 'enhanced_excruciate', 22, 48, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27322, 'swift_necrogenesis', 22, 24, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27323, 'enhanced_parsimony', 22, 16, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27324, 'enhanced_sanguine_rite', 22, 44, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27325, 'enhanced_enfeebling_magic', 22, 12, 26, 8);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27326, 'enhanced_enfeebling_magic_ii', 22, 28, 26, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27327, 'enhanced_magic_potency', 22, 8, 23, 8);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27328, 'enhanced_magic_potency_ii', 22, 28, 23, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27329, 'enhanced_magic_crit_potency', 22, 40, 37, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27330, 'auto-refresh', 22, 20, 49, 3);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27360, 'swift_sacred_prism', 23, 40, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27361, 'swift_shroud_of_saints', 23, 44, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27362, 'enhanced_blissful_mind', 23, 32, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27363, 'enhanced_raise', 23, 48, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27364, 'enhanced_stoneskin', 23, 36, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27365, 'enhanced_protect', 23, 24, 0, 0);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27366, 'greater_enhancing_magic', 23, 12, 25, 8);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27367, 'greater_healing', 23, 8, 24, 8);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27368, 'greater_healing_ii', 23, 18, 24, 10);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27369, 'enhanced_magic_accuracy', 23, 16, 27, 8);
INSERT OR IGNORE INTO "server_battle_traits" ("id", "name", "classJob", "lvl", "modifier", "bonus") VALUES
    (27370, 'auto-refresh', 23, 20, 49, 3);
