-- Per-item drop metadata for gathering nodes. One row per "item key"
-- referenced from `gamedata_gather_nodes.item1..item11`.
--
-- Column meanings mirror the old hardcoded `harvestNodeItems` Lua table:
--   itemCatalogId — the 1.x catalog id that lands in the player bag
--                   on a successful strike (e.g. 10001006 = Copper Ore)
--   remainder     — node HP pool at the start of this item's strike
--                   phase. `DummyCommand.lua` decrements by 20 per swing.
--                   Classic values: 40 / 60 / 70 / 80 (labelled A..D).
--   aim           — 0..100 slider position that selects this item when
--                   the player commits. Rounds down to one of 11 slots
--                   (aim/10 + 1) in the minigame.
--   sweetspot     — 0..100 power-bar target for the strike phase.
--                   `powerRange` (±10) is the width of the "hit" band.
--   maxYield      — maximum quantity granted on a perfect strike.

DROP TABLE IF EXISTS "gamedata_gather_node_items";
CREATE TABLE IF NOT EXISTS "gamedata_gather_node_items" (
    "id"             INTEGER PRIMARY KEY,
    "itemCatalogId"  INTEGER NOT NULL,
    "remainder"      INTEGER NOT NULL DEFAULT 80,
    "aim"            INTEGER NOT NULL DEFAULT 50,
    "sweetspot"      INTEGER NOT NULL DEFAULT 30,
    "maxYield"       INTEGER NOT NULL DEFAULT 1
);

INSERT OR IGNORE INTO "gamedata_gather_node_items"
    ("id", "itemCatalogId", "remainder", "aim", "sweetspot", "maxYield")
VALUES
    (1,    10009104, 70, 30, 30, 4),   -- Rock Salt
    (2,    10006001, 80, 10, 30, 4),   -- Bone Chip
    (3,    10001006, 80, 20, 30, 3),   -- Copper Ore
    (3001, 10001003, 80, 50, 30, 3),
    (3002, 10001006, 70, 70, 10, 4),
    (3003, 10001005, 80, 90, 70, 1),
    (3004, 10009104, 40, 10, 100, 2),
    (3005, 10001007, 40,  0, 30, 1);

-- -------------------------------------------------------------------
-- Mozk-tabetai 1.x reseed. 531 rows, IDs 5000..5530 — one per
-- (command, place, item) triple in mozk's gather table. Generated
-- from mozk-raw.json via the same one-shot emitter that produced
-- the 044 block; refresh both seeds together when regenerating.
--
-- Encoding of the `aim` column:
--   mozk's `gatherAim.aim` is a signed level in the retail range
--   -5..+5 (11 discrete values). garlemald's `aim` column is the
--   0..100 slider position the retail minigame widget reads. The
--   mapping is `aim = (aim_level + 5) * 10`, so -5 → 0 (leftmost
--   slot) and +5 → 100 (rightmost slot). Items without a
--   `gatherAim` row default to aim=50 (middle slot).
--
-- Defaults on un-sourced fields: remainder=80, sweetspot=30,
-- maxYield=3. Real values vary per retail node; mozk does not
-- publish them. Swap in per-item overrides if a canonical source
-- (retail dat-table, Meteor spreadsheet) turns up later.
-- -------------------------------------------------------------------

INSERT OR IGNORE INTO "gamedata_gather_node_items"
    ("id", "itemCatalogId", "remainder", "aim", "sweetspot", "maxYield")
VALUES
    (5000, 10001001, 80, 60, 30, 3),  -- Mine @ Bearded Rock — Tin Ore
    (5001, 10009101, 80, 50, 30, 3),  -- Mine @ Bearded Rock — Brimstone
    (5002, 10009111, 80, 40, 30, 3),  -- Mine @ Bearded Rock — Alumen
    (5003, 10001004, 80, 30, 30, 3),  -- Mine @ Skull Valley — Iron Ore
    (5004, 10004002, 80, 70, 30, 3),  -- Mine @ Skull Valley — Raw Lapis Lazuli
    (5005, 10004005, 80, 80, 30, 3),  -- Mine @ Skull Valley — Raw Fluorite
    (5006, 10006112, 80, 90, 30, 3),  -- Mine @ Skull Valley — Sunrise Tellin
    (5007, 10009111, 80, 40, 30, 3),  -- Mine @ Skull Valley — Alumen
    (5008, 3940002, 80, 90, 30, 3),  -- Mine @ Bloodshore — Lugworm
    (5009, 10001004, 80, 30, 30, 3),  -- Mine @ Bloodshore — Iron Ore
    (5010, 10004008, 80, 20, 30, 3),  -- Mine @ Bloodshore — Raw Aquamarine
    (5011, 10004011, 80, 80, 30, 3),  -- Mine @ Bloodshore — Raw Amethyst
    (5012, 10009101, 80, 50, 30, 3),  -- Mine @ Bloodshore — Brimstone
    (5013, 10001004, 80, 30, 30, 3),  -- Mine @ Iron Lake — Iron Ore
    (5014, 10001013, 80, 10, 30, 3),  -- Mine @ Iron Lake — Cobalt Ore
    (5015, 10004014, 80, 70, 30, 3),  -- Mine @ Iron Lake — Raw Turquoise
    (5016, 10004017, 80, 80, 30, 3),  -- Mine @ Iron Lake — Raw Spinel
    (5017, 10009101, 80, 50, 30, 3),  -- Mine @ Iron Lake — Brimstone
    (5018, 10001004, 80, 30, 30, 3),  -- Mine @ Cedarwood — Iron Ore
    (5019, 10001013, 80, 10, 30, 3),  -- Mine @ Cedarwood — Cobalt Ore
    (5020, 10004014, 80, 70, 30, 3),  -- Mine @ Cedarwood — Raw Turquoise
    (5021, 10004017, 80, 80, 30, 3),  -- Mine @ Cedarwood — Raw Spinel
    (5022, 10009101, 80, 50, 30, 3),  -- Mine @ Cedarwood — Brimstone
    (5023, 10001001, 80, 60, 30, 3),  -- Mine @ Bentbranch — Tin Ore
    (5024, 10009102, 80, 10, 30, 3),  -- Mine @ Bentbranch — Silex
    (5025, 10009108, 80, 20, 30, 3),  -- Mine @ Bentbranch — Minium
    (5026, 10001008, 80, 60, 30, 3),  -- Mine @ Emerald Moss — Zinc Ore
    (5027, 10004001, 80, 90, 30, 3),  -- Mine @ Emerald Moss — Raw Sunstone
    (5028, 10004004, 80, 10, 30, 3),  -- Mine @ Emerald Moss — Raw Malachite
    (5029, 10009102, 80, 10, 30, 3),  -- Mine @ Emerald Moss — Silex
    (5030, 10009108, 80, 20, 30, 3),  -- Mine @ Emerald Moss — Minium
    (5031, 10001008, 80, 60, 30, 3),  -- Mine @ Tranquil Paths — Zinc Ore
    (5032, 10001009, 80, 70, 30, 3),  -- Mine @ Tranquil Paths — Silver Ore
    (5033, 10004007, 80, 90, 30, 3),  -- Mine @ Tranquil Paths — Raw Garnet
    (5034, 10004010, 80, 10, 30, 3),  -- Mine @ Tranquil Paths — Raw Peridot
    (5035, 10009111, 80, 40, 30, 3),  -- Mine @ Tranquil Paths — Alumen
    (5036, 10001009, 80, 70, 30, 3),  -- Mine @ Humblehearth — Silver Ore
    (5037, 10001010, 80, 80, 30, 3),  -- Mine @ Humblehearth — Mythril Ore
    (5038, 10004013, 80, 90, 30, 3),  -- Mine @ Humblehearth — Raw Rubellite
    (5039, 10004016, 80, 10, 30, 3),  -- Mine @ Humblehearth — Raw Tourmaline
    (5040, 10009111, 80, 40, 30, 3),  -- Mine @ Humblehearth — Alumen
    (5041, 10001009, 80, 70, 30, 3),  -- Mine @ Treespeak — Silver Ore
    (5042, 10001010, 80, 80, 30, 3),  -- Mine @ Treespeak — Mythril Ore
    (5043, 10004013, 80, 90, 30, 3),  -- Mine @ Treespeak — Raw Rubellite
    (5044, 10004016, 80, 10, 30, 3),  -- Mine @ Treespeak — Raw Tourmaline
    (5045, 10009111, 80, 40, 30, 3),  -- Mine @ Treespeak — Alumen
    (5046, 10013009, 80, 100, 30, 3),  -- Mine @ Treespeak — Crystallized Matter
    (5047, 10001006, 80, 80, 30, 3),  -- Mine @ Black Brush — Copper Ore
    (5048, 10006001, 80, 90, 30, 3),  -- Mine @ Black Brush — Bone Chip
    (5049, 10009104, 80, 70, 30, 3),  -- Mine @ Black Brush — Rock Salt
    (5050, 10001006, 80, 80, 30, 3),  -- Mine @ Drybone — Copper Ore
    (5051, 10004003, 80, 50, 30, 3),  -- Mine @ Drybone — Raw Sphene
    (5052, 10004006, 80, 60, 30, 3),  -- Mine @ Drybone — Raw Danburite
    (5053, 10006001, 80, 90, 30, 3),  -- Mine @ Drybone — Bone Chip
    (5054, 10006003, 80, 10, 30, 3),  -- Mine @ Drybone — Soiled Femur
    (5055, 10001006, 80, 80, 30, 3),  -- Mine @ Horizon's Edge — Copper Ore
    (5056, 10004009, 80, 50, 30, 3),  -- Mine @ Horizon's Edge — Raw Heliodor
    (5057, 10004012, 80, 60, 30, 3),  -- Mine @ Horizon's Edge — Raw Goshenite
    (5058, 10006003, 80, 10, 30, 3),  -- Mine @ Horizon's Edge — Soiled Femur
    (5059, 10009111, 80, 40, 30, 3),  -- Mine @ Horizon's Edge — Alumen
    (5060, 10013006, 80, 100, 30, 3),  -- Mine @ Horizon's Edge — Carbonized Matter
    (5061, 10001004, 80, 30, 30, 3),  -- Mine @ Halatali — Iron Ore
    (5062, 10001011, 80, 10, 30, 3),  -- Mine @ Halatali — Gold Ore
    (5063, 10001014, 80, 90, 30, 3),  -- Mine @ Halatali — Electrum Ore
    (5064, 10004015, 80, 50, 30, 3),  -- Mine @ Halatali — Raw Amber
    (5065, 10004018, 80, 60, 30, 3),  -- Mine @ Halatali — Raw Zircon
    (5066, 10001004, 80, 30, 30, 3),  -- Mine @ Nophica's Wells — Iron Ore
    (5067, 10001014, 80, 90, 30, 3),  -- Mine @ Nophica's Wells — Electrum Ore
    (5068, 10004015, 80, 50, 30, 3),  -- Mine @ Nophica's Wells — Raw Amber
    (5069, 10004018, 80, 60, 30, 3),  -- Mine @ Nophica's Wells — Raw Zircon
    (5070, 10009111, 80, 40, 30, 3),  -- Mine @ Nophica's Wells — Alumen
    (5071, 10013007, 80, 100, 30, 3),  -- Mine @ Nophica's Wells — Petrified Matter
    (5072, 10001004, 80, 30, 30, 3),  -- Mine @ Nanawa Mines — Iron Ore
    (5073, 10001009, 80, 70, 30, 3),  -- Mine @ Nanawa Mines — Silver Ore
    (5074, 10011197, 80, 10, 30, 3),  -- Mine @ Nanawa Mines — Light Kidney Ore
    (5075, 10011208, 80, 40, 30, 3),  -- Mine @ Nanawa Mines — Stiperstone
    (5076, 10001005, 80, 20, 30, 3),  -- Mine @ Dragonhead — Darksteel Ore
    (5077, 10001009, 80, 70, 30, 3),  -- Mine @ Dragonhead — Silver Ore
    (5078, 10001010, 80, 80, 30, 3),  -- Mine @ Dragonhead — Mythril Ore
    (5079, 10004026, 80, 30, 30, 3),  -- Mine @ Dragonhead — Jade
    (5080, 10013008, 80, 100, 30, 3),  -- Mine @ Dragonhead — Fossilized Matter
    (5081, 10001008, 80, 60, 30, 3),  -- Mine @ The Fields of Glory — Zinc Ore
    (5082, 10001009, 80, 70, 30, 3),  -- Mine @ The Fields of Glory — Silver Ore
    (5083, 10001010, 80, 80, 30, 3),  -- Mine @ The Fields of Glory — Mythril Ore
    (5084, 3011309, 80, 20, 30, 3),  -- Log @ Bearded Rock — Coerthas Carrot
    (5085, 3011451, 80, 80, 30, 3),  -- Log @ Bearded Rock — La Noscean Orange
    (5086, 10005401, 80, 40, 30, 3),  -- Log @ Bearded Rock — Cock Feather
    (5087, 10008005, 80, 30, 30, 3),  -- Log @ Bearded Rock — Ash Log
    (5088, 10008106, 80, 20, 30, 3),  -- Log @ Bearded Rock — Ash Branch
    (5089, 3011404, 80, 60, 30, 3),  -- Log @ Skull Valley — Cinderfoot Olive
    (5090, 10005401, 80, 40, 30, 3),  -- Log @ Skull Valley — Cock Feather
    (5091, 10008005, 80, 30, 30, 3),  -- Log @ Skull Valley — Ash Log
    (5092, 10008106, 80, 20, 30, 3),  -- Log @ Skull Valley — Ash Branch
    (5093, 3011403, 80, 40, 30, 3),  -- Log @ Bloodshore — Gridanian Walnut
    (5094, 3011404, 80, 60, 30, 3),  -- Log @ Bloodshore — Cinderfoot Olive
    (5095, 3011518, 80, 10, 30, 3),  -- Log @ Bloodshore — Cloves
    (5096, 10008007, 80, 20, 30, 3),  -- Log @ Bloodshore — Walnut Log
    (5097, 3011456, 80, 30, 30, 3),  -- Log @ Iron Lake — Sun Lemon
    (5098, 3011510, 80, 0, 30, 3),  -- Log @ Iron Lake — Sagolii Sage
    (5099, 10008011, 80, 40, 30, 3),  -- Log @ Iron Lake — Oak Log
    (5100, 10008016, 80, 80, 30, 3),  -- Log @ Iron Lake — Mahogany Log
    (5101, 10013012, 80, 100, 30, 3),  -- Log @ Iron Lake — Decomposed Matter
    (5102, 3011403, 80, 40, 30, 3),  -- Log @ Cedarwood — Gridanian Walnut
    (5103, 3011456, 80, 30, 30, 3),  -- Log @ Cedarwood — Sun Lemon
    (5104, 3011512, 80, 20, 30, 3),  -- Log @ Cedarwood — Dragon Pepper
    (5105, 10008007, 80, 20, 30, 3),  -- Log @ Cedarwood — Walnut Log
    (5106, 10008003, 80, 30, 30, 3),  -- Log @ Bentbranch — Maple Log
    (5107, 10008104, 80, 40, 30, 3),  -- Log @ Bentbranch — Maple Branch
    (5108, 10008502, 80, 50, 30, 3),  -- Log @ Bentbranch — Maple Sap
    (5109, 10009610, 80, 60, 30, 3),  -- Log @ Bentbranch — Tinolqa Mistletoe
    (5110, 10008011, 80, 40, 30, 3),  -- Log @ Nine Ivies — Oak Log
    (5111, 10008112, 80, 50, 30, 3),  -- Log @ Nine Ivies — Oak Branch
    (5112, 10011199, 80, 90, 30, 3),  -- Log @ Nine Ivies — Supple Spruce Branch
    (5113, 10011209, 80, 70, 30, 3),  -- Log @ Nine Ivies — Resin
    (5114, 3011403, 80, 40, 30, 3),  -- Log @ Emerald Moss — Gridanian Walnut
    (5115, 3011455, 80, 70, 30, 3),  -- Log @ Emerald Moss — Faerie Apple
    (5116, 10008003, 80, 30, 30, 3),  -- Log @ Emerald Moss — Maple Log
    (5117, 10008104, 80, 40, 30, 3),  -- Log @ Emerald Moss — Maple Branch
    (5118, 10008502, 80, 50, 30, 3),  -- Log @ Emerald Moss — Maple Sap
    (5119, 10009405, 80, 30, 30, 3),  -- Log @ Emerald Moss — Lavender
    (5120, 10005404, 80, 20, 30, 3),  -- Log @ Tranquil Paths — Wildfowl Feather
    (5121, 10008008, 80, 80, 30, 3),  -- Log @ Tranquil Paths — Yew Log
    (5122, 10008109, 80, 70, 30, 3),  -- Log @ Tranquil Paths — Yew Branch
    (5123, 10009611, 80, 60, 30, 3),  -- Log @ Tranquil Paths — Matron's Mistletoe
    (5124, 10013010, 80, 100, 30, 3),  -- Log @ Tranquil Paths — Germinated Matter
    (5125, 3011304, 80, 40, 30, 3),  -- Log @ Humblehearth — Salt Leek
    (5126, 3011401, 80, 30, 30, 3),  -- Log @ Humblehearth — Iron Acorn
    (5127, 10008008, 80, 80, 30, 3),  -- Log @ Humblehearth — Yew Log
    (5128, 10008109, 80, 70, 30, 3),  -- Log @ Humblehearth — Yew Branch
    (5129, 10013011, 80, 100, 30, 3),  -- Log @ Humblehearth — Decayed Matter
    (5130, 10005404, 80, 20, 30, 3),  -- Log @ Treespeak — Wildfowl Feather
    (5131, 10008011, 80, 40, 30, 3),  -- Log @ Treespeak — Oak Log
    (5132, 10008112, 80, 50, 30, 3),  -- Log @ Treespeak — Oak Branch
    (5133, 10009402, 80, 60, 30, 3),  -- Log @ Treespeak — Mistletoe
    (5134, 3011308, 80, 40, 30, 3),  -- Log @ Black Brush — Wild Onion
    (5135, 3011416, 80, 20, 30, 3),  -- Log @ Black Brush — Kukuru Bean
    (5136, 10005403, 80, 30, 30, 3),  -- Log @ Black Brush — Crow Feather
    (5137, 10008004, 80, 50, 30, 3),  -- Log @ Black Brush — Elm Log
    (5138, 10009306, 80, 60, 30, 3),  -- Log @ Black Brush — Latex
    (5139, 10009407, 80, 90, 30, 3),  -- Log @ Black Brush — Yellow Ginseng
    (5140, 3011319, 80, 80, 30, 3),  -- Log @ Drybone — Nopales
    (5141, 10005403, 80, 30, 30, 3),  -- Log @ Drybone — Crow Feather
    (5142, 10008004, 80, 50, 30, 3),  -- Log @ Drybone — Elm Log
    (5143, 10008005, 80, 30, 30, 3),  -- Log @ Drybone — Ash Log
    (5144, 10009306, 80, 60, 30, 3),  -- Log @ Drybone — Latex
    (5145, 10009406, 80, 30, 30, 3),  -- Log @ Drybone — Belladonna
    (5146, 3011318, 80, 60, 30, 3),  -- Log @ Horizon's Edge — Aloe
    (5147, 10008004, 80, 50, 30, 3),  -- Log @ Horizon's Edge — Elm Log
    (5148, 10009306, 80, 60, 30, 3),  -- Log @ Horizon's Edge — Latex
    (5149, 3011512, 80, 20, 30, 3),  -- Log @ Halatali — Dragon Pepper
    (5150, 10008014, 80, 30, 30, 3),  -- Log @ Halatali — Rosewood Log
    (5151, 10008115, 80, 40, 30, 3),  -- Log @ Halatali — Rosewood Branch
    (5152, 10013013, 80, 100, 30, 3),  -- Log @ Halatali — Liquefied Matter
    (5153, 3011512, 80, 20, 30, 3),  -- Log @ Nophica's Wells — Dragon Pepper
    (5154, 3011515, 80, 20, 30, 3),  -- Log @ Nophica's Wells — Nutmeg
    (5155, 10008008, 80, 80, 30, 3),  -- Log @ Nophica's Wells — Yew Log
    (5156, 10008109, 80, 70, 30, 3),  -- Log @ Nophica's Wells — Yew Branch
    (5157, 3011401, 80, 30, 30, 3),  -- Log @ Dragonhead — Iron Acorn
    (5158, 10005410, 80, 80, 30, 3),  -- Log @ Dragonhead — Chocobo Feather
    (5159, 10008013, 80, 20, 30, 3),  -- Log @ Dragonhead — Spruce Log
    (5160, 10009402, 80, 60, 30, 3),  -- Log @ Dragonhead — Mistletoe
    (5161, 10005410, 80, 80, 30, 3),  -- Log @ The Fields of Glory — Chocobo Feather
    (5162, 10008011, 80, 40, 30, 3),  -- Log @ The Fields of Glory — Oak Log
    (5163, 10008112, 80, 50, 30, 3),  -- Log @ The Fields of Glory — Oak Branch
    (5164, 3011205, 80, 70, 30, 3),  -- Fish @ Bearded Rock — Tiger Cod
    (5165, 3011210, 80, 20, 30, 3),  -- Fish @ Bearded Rock — Merlthor Goby
    (5166, 3011216, 80, 10, 30, 3),  -- Fish @ Bearded Rock — Malm Kelp
    (5167, 3011228, 80, 10, 30, 3),  -- Fish @ Bearded Rock — Sea Cucumber
    (5168, 3011231, 80, 10, 30, 3),  -- Fish @ Bearded Rock — Vongola Clam
    (5169, 10006108, 80, 20, 30, 3),  -- Fish @ Bearded Rock — White Coral
    (5170, 3011129, 80, 10, 30, 3),  -- Fish @ Skull Valley — Helmet Crab
    (5171, 3011203, 80, 30, 30, 3),  -- Fish @ Skull Valley — Nautilus
    (5172, 3011205, 80, 70, 30, 3),  -- Fish @ Skull Valley — Tiger Cod
    (5173, 3011208, 80, 80, 30, 3),  -- Fish @ Skull Valley — Coral Butterfly
    (5174, 3011213, 80, 10, 30, 3),  -- Fish @ Skull Valley — Rothlyt Oyster
    (5175, 3011216, 80, 10, 30, 3),  -- Fish @ Skull Valley — Malm Kelp
    (5176, 3011228, 80, 10, 30, 3),  -- Fish @ Skull Valley — Sea Cucumber
    (5177, 3011230, 80, 10, 30, 3),  -- Fish @ Skull Valley — Razor Clam
    (5178, 10006108, 80, 20, 30, 3),  -- Fish @ Skull Valley — White Coral
    (5179, 3011201, 80, 70, 30, 3),  -- Fish @ Bald Knoll — Saber Sardine
    (5180, 3011216, 80, 10, 30, 3),  -- Fish @ Bald Knoll — Malm Kelp
    (5181, 3011224, 80, 60, 30, 3),  -- Fish @ Bald Knoll — Wahoo
    (5182, 3011227, 80, 70, 30, 3),  -- Fish @ Bald Knoll — Haraldr Haddock
    (5183, 10006108, 80, 20, 30, 3),  -- Fish @ Bald Knoll — White Coral
    (5184, 10011198, 80, 80, 30, 3),  -- Fish @ Bald Knoll — Young Indigo Herring
    (5185, 10011210, 80, 50, 30, 3),  -- Fish @ Bald Knoll — Navigator's Ear
    (5186, 10013016, 80, 0, 30, 3),  -- Fish @ Bald Knoll — Ossified Matter
    (5187, 3011203, 80, 30, 30, 3),  -- Fish @ Bloodshore — Nautilus
    (5188, 3011209, 80, 60, 30, 3),  -- Fish @ Bloodshore — Bianaq Bream
    (5189, 3011213, 80, 10, 30, 3),  -- Fish @ Bloodshore — Rothlyt Oyster
    (5190, 3011216, 80, 10, 30, 3),  -- Fish @ Bloodshore — Malm Kelp
    (5191, 3011229, 80, 10, 30, 3),  -- Fish @ Bloodshore — Sea Pickle
    (5192, 10013014, 80, 0, 30, 3),  -- Fish @ Bloodshore — Calcified Matter
    (5193, 3011102, 80, 60, 30, 3),  -- Fish @ Iron Lake — Dark Bass
    (5194, 3011103, 80, 30, 30, 3),  -- Fish @ Iron Lake — Crayfish
    (5195, 3011104, 80, 10, 30, 3),  -- Fish @ Iron Lake — Crimson Crayfish
    (5196, 3011106, 80, 70, 30, 3),  -- Fish @ Iron Lake — Maiden Carp
    (5197, 3011123, 80, 10, 30, 3),  -- Fish @ Iron Lake — Lava Toad
    (5198, 3011204, 80, 60, 30, 3),  -- Fish @ Cedarwood — Silver Shark
    (5199, 3011209, 80, 60, 30, 3),  -- Fish @ Cedarwood — Bianaq Bream
    (5200, 3011211, 80, 80, 30, 3),  -- Fish @ Cedarwood — Indigo Herring
    (5201, 3011212, 80, 70, 30, 3),  -- Fish @ Cedarwood — Blowfish
    (5202, 3011216, 80, 10, 30, 3),  -- Fish @ Cedarwood — Malm Kelp
    (5203, 3011217, 80, 50, 30, 3),  -- Fish @ Cedarwood — Ash Tuna
    (5204, 3011225, 80, 60, 30, 3),  -- Fish @ Cedarwood — Hammerhead Shark
    (5205, 3011227, 80, 70, 30, 3),  -- Fish @ Cedarwood — Haraldr Haddock
    (5206, 10006109, 80, 20, 30, 3),  -- Fish @ Cedarwood — Blue Coral
    (5207, 10006110, 80, 20, 30, 3),  -- Fish @ Cedarwood — Red Coral
    (5208, 10013015, 80, 0, 30, 3),  -- Fish @ Cedarwood — Cultured Matter
    (5209, 3011202, 80, 90, 30, 3),  -- Fish @ Limsa Lominsa — Ocean Cloud
    (5210, 3011205, 80, 70, 30, 3),  -- Fish @ Limsa Lominsa — Tiger Cod
    (5211, 3011210, 80, 20, 30, 3),  -- Fish @ Limsa Lominsa — Merlthor Goby
    (5212, 3011216, 80, 10, 30, 3),  -- Fish @ Limsa Lominsa — Malm Kelp
    (5213, 3011231, 80, 10, 30, 3),  -- Fish @ Limsa Lominsa — Vongola Clam
    (5214, 3011103, 80, 30, 30, 3),  -- Fish @ Mistbeard Cove — Crayfish
    (5215, 3011104, 80, 10, 30, 3),  -- Fish @ Mistbeard Cove — Crimson Crayfish
    (5216, 3011114, 80, 80, 30, 3),  -- Fish @ Mistbeard Cove — Blindfish
    (5217, 3011117, 80, 20, 30, 3),  -- Fish @ Mistbeard Cove — Lamp Marimo
    (5218, 3011118, 80, 10, 30, 3),  -- Fish @ Mistbeard Cove — Monke Onke
    (5219, 3011103, 80, 30, 30, 3),  -- Fish @ Cassiopeia Hollow — Crayfish
    (5220, 3011104, 80, 10, 30, 3),  -- Fish @ Cassiopeia Hollow — Crimson Crayfish
    (5221, 3011114, 80, 80, 30, 3),  -- Fish @ Cassiopeia Hollow — Blindfish
    (5222, 3011117, 80, 20, 30, 3),  -- Fish @ Cassiopeia Hollow — Lamp Marimo
    (5223, 3011134, 80, 20, 30, 3),  -- Fish @ Cassiopeia Hollow — Nether Newt
    (5224, 3011103, 80, 30, 30, 3),  -- Fish @ Gridania — Crayfish
    (5225, 3011108, 80, 20, 30, 3),  -- Fish @ Gridania — Pipira
    (5226, 3011110, 80, 90, 30, 3),  -- Fish @ Gridania — Brass Loach
    (5227, 3011112, 80, 30, 30, 3),  -- Fish @ Gridania — Rainbow Trout
    (5228, 3011117, 80, 20, 30, 3),  -- Fish @ Gridania — Lamp Marimo
    (5229, 3011132, 80, 90, 30, 3),  -- Fish @ Gridania — Tree Toad
    (5230, 3011103, 80, 30, 30, 3),  -- Fish @ Bentbranch — Crayfish
    (5231, 3011108, 80, 20, 30, 3),  -- Fish @ Bentbranch — Pipira
    (5232, 3011110, 80, 90, 30, 3),  -- Fish @ Bentbranch — Brass Loach
    (5233, 3011111, 80, 10, 30, 3),  -- Fish @ Bentbranch — Ala Mhigan Fighting Fish
    (5234, 3011112, 80, 30, 30, 3),  -- Fish @ Bentbranch — Rainbow Trout
    (5235, 3011132, 80, 90, 30, 3),  -- Fish @ Bentbranch — Tree Toad
    (5236, 3011133, 80, 90, 30, 3),  -- Fish @ Bentbranch — Dart Frog
    (5237, 3011103, 80, 30, 30, 3),  -- Fish @ Nine Ivies — Crayfish
    (5238, 3011107, 80, 80, 30, 3),  -- Fish @ Nine Ivies — Velodyna Carp
    (5239, 3011108, 80, 20, 30, 3),  -- Fish @ Nine Ivies — Pipira
    (5240, 3011118, 80, 10, 30, 3),  -- Fish @ Nine Ivies — Monke Onke
    (5241, 3011120, 80, 40, 30, 3),  -- Fish @ Nine Ivies — Northern Pike
    (5242, 10013017, 80, 0, 30, 3),  -- Fish @ Nine Ivies — Cretified Matter
    (5243, 3011101, 80, 30, 30, 3),  -- Fish @ Emerald Moss — Black Eel
    (5244, 3011103, 80, 30, 30, 3),  -- Fish @ Emerald Moss — Crayfish
    (5245, 3011108, 80, 20, 30, 3),  -- Fish @ Emerald Moss — Pipira
    (5246, 3011111, 80, 10, 30, 3),  -- Fish @ Emerald Moss — Ala Mhigan Fighting Fish
    (5247, 3011113, 80, 30, 30, 3),  -- Fish @ Emerald Moss — Yugr'am Salmon
    (5248, 3011128, 80, 10, 30, 3),  -- Fish @ Emerald Moss — River Crab
    (5249, 3011133, 80, 90, 30, 3),  -- Fish @ Emerald Moss — Dart Frog
    (5250, 3011136, 80, 10, 30, 3),  -- Fish @ Emerald Moss — Box Turtle
    (5251, 3011103, 80, 30, 30, 3),  -- Fish @ Tranquil Paths — Crayfish
    (5252, 3011105, 80, 10, 30, 3),  -- Fish @ Tranquil Paths — Bone Crayfish
    (5253, 3011109, 80, 40, 30, 3),  -- Fish @ Tranquil Paths — Black Ghost
    (5254, 3011112, 80, 30, 30, 3),  -- Fish @ Tranquil Paths — Rainbow Trout
    (5255, 3011113, 80, 30, 30, 3),  -- Fish @ Tranquil Paths — Yugr'am Salmon
    (5256, 3011128, 80, 10, 30, 3),  -- Fish @ Tranquil Paths — River Crab
    (5257, 3011136, 80, 10, 30, 3),  -- Fish @ Tranquil Paths — Box Turtle
    (5258, 3011103, 80, 30, 30, 3),  -- Fish @ Humblehearth — Crayfish
    (5259, 3011105, 80, 10, 30, 3),  -- Fish @ Humblehearth — Bone Crayfish
    (5260, 3011113, 80, 30, 30, 3),  -- Fish @ Humblehearth — Yugr'am Salmon
    (5261, 3011118, 80, 10, 30, 3),  -- Fish @ Humblehearth — Monke Onke
    (5262, 3011121, 80, 40, 30, 3),  -- Fish @ Humblehearth — Southern Pike
    (5263, 3011103, 80, 30, 30, 3),  -- Fish @ The Mun Tuy Cellars — Crayfish
    (5264, 3011105, 80, 10, 30, 3),  -- Fish @ The Mun Tuy Cellars — Bone Crayfish
    (5265, 3011107, 80, 80, 30, 3),  -- Fish @ The Mun Tuy Cellars — Velodyna Carp
    (5266, 3011114, 80, 80, 30, 3),  -- Fish @ The Mun Tuy Cellars — Blindfish
    (5267, 3011118, 80, 10, 30, 3),  -- Fish @ The Mun Tuy Cellars — Monke Onke
    (5268, 3011103, 80, 30, 30, 3),  -- Fish @ The Tam Tara Deepcroft — Crayfish
    (5269, 3011105, 80, 10, 30, 3),  -- Fish @ The Tam Tara Deepcroft — Bone Crayfish
    (5270, 3011107, 80, 80, 30, 3),  -- Fish @ The Tam Tara Deepcroft — Velodyna Carp
    (5271, 3011114, 80, 80, 30, 3),  -- Fish @ The Tam Tara Deepcroft — Blindfish
    (5272, 3011118, 80, 10, 30, 3),  -- Fish @ The Tam Tara Deepcroft — Monke Onke
    (5273, 3011103, 80, 30, 30, 3),  -- Fish @ Black Brush — Crayfish
    (5274, 3011106, 80, 70, 30, 3),  -- Fish @ Black Brush — Maiden Carp
    (5275, 3011108, 80, 20, 30, 3),  -- Fish @ Black Brush — Pipira
    (5276, 3011110, 80, 90, 30, 3),  -- Fish @ Black Brush — Brass Loach
    (5277, 3011115, 80, 10, 30, 3),  -- Fish @ Black Brush — Striped Goby
    (5278, 3011116, 80, 20, 30, 3),  -- Fish @ Black Brush — Sandfish
    (5279, 3011101, 80, 30, 30, 3),  -- Fish @ Drybone — Black Eel
    (5280, 3011102, 80, 60, 30, 3),  -- Fish @ Drybone — Dark Bass
    (5281, 3011103, 80, 30, 30, 3),  -- Fish @ Drybone — Crayfish
    (5282, 3011106, 80, 70, 30, 3),  -- Fish @ Drybone — Maiden Carp
    (5283, 3011108, 80, 20, 30, 3),  -- Fish @ Drybone — Pipira
    (5284, 3011111, 80, 10, 30, 3),  -- Fish @ Drybone — Ala Mhigan Fighting Fish
    (5285, 3011115, 80, 10, 30, 3),  -- Fish @ Drybone — Striped Goby
    (5286, 3011116, 80, 20, 30, 3),  -- Fish @ Drybone — Sandfish
    (5287, 3011102, 80, 60, 30, 3),  -- Fish @ Horizon's Edge — Dark Bass
    (5288, 3011103, 80, 30, 30, 3),  -- Fish @ Horizon's Edge — Crayfish
    (5289, 3011109, 80, 40, 30, 3),  -- Fish @ Horizon's Edge — Black Ghost
    (5290, 3011128, 80, 10, 30, 3),  -- Fish @ Horizon's Edge — River Crab
    (5291, 3011136, 80, 10, 30, 3),  -- Fish @ Horizon's Edge — Box Turtle
    (5292, 3011202, 80, 90, 30, 3),  -- Fish @ Horizon's Edge — Ocean Cloud
    (5293, 3011205, 80, 70, 30, 3),  -- Fish @ Horizon's Edge — Tiger Cod
    (5294, 3011207, 80, 10, 30, 3),  -- Fish @ Horizon's Edge — Black Sole
    (5295, 3011216, 80, 10, 30, 3),  -- Fish @ Horizon's Edge — Malm Kelp
    (5296, 3011225, 80, 60, 30, 3),  -- Fish @ Horizon's Edge — Hammerhead Shark
    (5297, 3011103, 80, 30, 30, 3),  -- Fish @ Broken Water — Crayfish
    (5298, 3011105, 80, 10, 30, 3),  -- Fish @ Broken Water — Bone Crayfish
    (5299, 3011107, 80, 80, 30, 3),  -- Fish @ Broken Water — Velodyna Carp
    (5300, 3011109, 80, 40, 30, 3),  -- Fish @ Broken Water — Black Ghost
    (5301, 3011111, 80, 10, 30, 3),  -- Fish @ Broken Water — Ala Mhigan Fighting Fish
    (5302, 3011101, 80, 30, 30, 3),  -- Fish @ Halatali — Black Eel
    (5303, 3011103, 80, 30, 30, 3),  -- Fish @ Halatali — Crayfish
    (5304, 3011104, 80, 10, 30, 3),  -- Fish @ Halatali — Crimson Crayfish
    (5305, 3011105, 80, 10, 30, 3),  -- Fish @ Halatali — Bone Crayfish
    (5306, 3011107, 80, 80, 30, 3),  -- Fish @ Halatali — Velodyna Carp
    (5307, 3011109, 80, 40, 30, 3),  -- Fish @ Halatali — Black Ghost
    (5308, 3011111, 80, 10, 30, 3),  -- Fish @ Halatali — Ala Mhigan Fighting Fish
    (5309, 3011115, 80, 10, 30, 3),  -- Fish @ Halatali — Striped Goby
    (5310, 3011103, 80, 30, 30, 3),  -- Fish @ Nophica's Wells — Crayfish
    (5311, 3011104, 80, 10, 30, 3),  -- Fish @ Nophica's Wells — Crimson Crayfish
    (5312, 3011105, 80, 10, 30, 3),  -- Fish @ Nophica's Wells — Bone Crayfish
    (5313, 3011111, 80, 10, 30, 3),  -- Fish @ Nophica's Wells — Ala Mhigan Fighting Fish
    (5314, 3011103, 80, 30, 30, 3),  -- Fish @ Ul'dah — Crayfish
    (5315, 3011106, 80, 70, 30, 3),  -- Fish @ Ul'dah — Maiden Carp
    (5316, 3011108, 80, 20, 30, 3),  -- Fish @ Ul'dah — Pipira
    (5317, 3011110, 80, 90, 30, 3),  -- Fish @ Ul'dah — Brass Loach
    (5318, 3011115, 80, 10, 30, 3),  -- Fish @ Ul'dah — Striped Goby
    (5319, 3011103, 80, 30, 30, 3),  -- Fish @ Dragonhead — Crayfish
    (5320, 3011104, 80, 10, 30, 3),  -- Fish @ Dragonhead — Crimson Crayfish
    (5321, 3011107, 80, 80, 30, 3),  -- Fish @ Dragonhead — Velodyna Carp
    (5322, 3011115, 80, 10, 30, 3),  -- Fish @ Dragonhead — Striped Goby
    (5323, 3011120, 80, 40, 30, 3),  -- Fish @ Dragonhead — Northern Pike
    (5324, 3011102, 80, 60, 30, 3),  -- Fish @ The Fields of Glory — Dark Bass
    (5325, 3011103, 80, 30, 30, 3),  -- Fish @ The Fields of Glory — Crayfish
    (5326, 3011105, 80, 10, 30, 3),  -- Fish @ The Fields of Glory — Bone Crayfish
    (5327, 3011118, 80, 10, 30, 3),  -- Fish @ The Fields of Glory — Monke Onke
    (5328, 3011136, 80, 10, 30, 3),  -- Fish @ The Fields of Glory — Box Turtle
    (5329, 3011103, 80, 30, 30, 3),  -- Fish @ Riversmeet — Crayfish
    (5330, 3011104, 80, 10, 30, 3),  -- Fish @ Riversmeet — Crimson Crayfish
    (5331, 3011107, 80, 80, 30, 3),  -- Fish @ Riversmeet — Velodyna Carp
    (5332, 3011115, 80, 10, 30, 3),  -- Fish @ Riversmeet — Striped Goby
    (5333, 3011120, 80, 40, 30, 3),  -- Fish @ Riversmeet — Northern Pike
    (5334, 10001116, 80, 50, 30, 3),  -- Quarry @ Bearded Rock — Ragstone
    (5335, 10010719, 80, 50, 30, 3),  -- Quarry @ Bearded Rock — All-purpose Red Dye
    (5336, 10001116, 80, 50, 30, 3),  -- Quarry @ Skull Valley — Ragstone
    (5337, 10010719, 80, 50, 30, 3),  -- Quarry @ Skull Valley — All-purpose Red Dye
    (5338, 10001116, 80, 50, 30, 3),  -- Quarry @ Bloodshore — Ragstone
    (5339, 10004029, 80, 50, 30, 3),  -- Quarry @ Bloodshore — Wind Rock
    (5340, 10004030, 80, 50, 30, 3),  -- Quarry @ Bloodshore — Water Rock
    (5341, 10010719, 80, 50, 30, 3),  -- Quarry @ Bloodshore — All-purpose Red Dye
    (5342, 10004029, 80, 50, 30, 3),  -- Quarry @ Iron Lake — Wind Rock
    (5343, 10004030, 80, 50, 30, 3),  -- Quarry @ Iron Lake — Water Rock
    (5344, 10010719, 80, 50, 30, 3),  -- Quarry @ Iron Lake — All-purpose Red Dye
    (5345, 10004029, 80, 50, 30, 3),  -- Quarry @ Cedarwood — Wind Rock
    (5346, 10004030, 80, 50, 30, 3),  -- Quarry @ Cedarwood — Water Rock
    (5347, 10010719, 80, 50, 30, 3),  -- Quarry @ Cedarwood — All-purpose Red Dye
    (5348, 10001102, 80, 50, 30, 3),  -- Quarry @ Bentbranch — Obsidian
    (5349, 10010725, 80, 50, 30, 3),  -- Quarry @ Bentbranch — All-purpose White Dye
    (5350, 10001102, 80, 50, 30, 3),  -- Quarry @ Emerald Moss — Obsidian
    (5351, 10010725, 80, 50, 30, 3),  -- Quarry @ Emerald Moss — All-purpose White Dye
    (5352, 10001102, 80, 50, 30, 3),  -- Quarry @ Tranquil Paths — Obsidian
    (5353, 10004031, 80, 50, 30, 3),  -- Quarry @ Tranquil Paths — Ice Rock
    (5354, 10004032, 80, 50, 30, 3),  -- Quarry @ Tranquil Paths — Earth Rock
    (5355, 10010725, 80, 50, 30, 3),  -- Quarry @ Tranquil Paths — All-purpose White Dye
    (5356, 10001117, 80, 50, 30, 3),  -- Quarry @ Humblehearth — Siltstone
    (5357, 10004031, 80, 50, 30, 3),  -- Quarry @ Humblehearth — Ice Rock
    (5358, 10004032, 80, 50, 30, 3),  -- Quarry @ Humblehearth — Earth Rock
    (5359, 10010725, 80, 50, 30, 3),  -- Quarry @ Humblehearth — All-purpose White Dye
    (5360, 10004031, 80, 50, 30, 3),  -- Quarry @ Treespeak — Ice Rock
    (5361, 10004032, 80, 50, 30, 3),  -- Quarry @ Treespeak — Earth Rock
    (5362, 10010725, 80, 50, 30, 3),  -- Quarry @ Treespeak — All-purpose White Dye
    (5363, 10001102, 80, 50, 30, 3),  -- Quarry @ Black Brush — Obsidian
    (5364, 10010720, 80, 50, 30, 3),  -- Quarry @ Black Brush — All-purpose Yellow Dye
    (5365, 10001102, 80, 50, 30, 3),  -- Quarry @ Drybone — Obsidian
    (5366, 10010720, 80, 50, 30, 3),  -- Quarry @ Drybone — All-purpose Yellow Dye
    (5367, 10001115, 80, 50, 30, 3),  -- Quarry @ Horizon's Edge — Mudstone
    (5368, 10004027, 80, 50, 30, 3),  -- Quarry @ Horizon's Edge — Fire Rock
    (5369, 10004028, 80, 50, 30, 3),  -- Quarry @ Horizon's Edge — Lightning Rock
    (5370, 10010720, 80, 50, 30, 3),  -- Quarry @ Horizon's Edge — All-purpose Yellow Dye
    (5371, 10004027, 80, 50, 30, 3),  -- Quarry @ Halatali — Fire Rock
    (5372, 10004028, 80, 50, 30, 3),  -- Quarry @ Halatali — Lightning Rock
    (5373, 10010720, 80, 50, 30, 3),  -- Quarry @ Halatali — All-purpose Yellow Dye
    (5374, 10001115, 80, 50, 30, 3),  -- Quarry @ Nophica's Wells — Mudstone
    (5375, 10004027, 80, 50, 30, 3),  -- Quarry @ Nophica's Wells — Fire Rock
    (5376, 10004028, 80, 50, 30, 3),  -- Quarry @ Nophica's Wells — Lightning Rock
    (5377, 10010720, 80, 50, 30, 3),  -- Quarry @ Nophica's Wells — All-purpose Yellow Dye
    (5378, 10001103, 80, 50, 30, 3),  -- Quarry @ Dragonhead — Wyvern Obsidian
    (5379, 10001117, 80, 50, 30, 3),  -- Quarry @ Dragonhead — Siltstone
    (5380, 10004242, 80, 50, 30, 3),  -- Quarry @ Dragonhead — Astral Rock
    (5381, 10001103, 80, 50, 30, 3),  -- Quarry @ The Fields of Glory — Wyvern Obsidian
    (5382, 10001115, 80, 50, 30, 3),  -- Quarry @ The Fields of Glory — Mudstone
    (5383, 10001117, 80, 50, 30, 3),  -- Quarry @ The Fields of Glory — Siltstone
    (5384, 3011312, 80, 50, 30, 3),  -- Harvest @ Bearded Rock — Highland Parsley
    (5385, 3011506, 80, 50, 30, 3),  -- Harvest @ Bearded Rock — Garlean Garlic
    (5386, 3011523, 80, 50, 30, 3),  -- Harvest @ Bearded Rock — Sunset Wheat
    (5387, 10005201, 80, 50, 30, 3),  -- Harvest @ Bearded Rock — Straw
    (5388, 10005202, 80, 50, 30, 3),  -- Harvest @ Bearded Rock — Moko Grass
    (5389, 10010718, 80, 50, 30, 3),  -- Harvest @ Bearded Rock — All-purpose Blue Dye
    (5390, 3011310, 80, 50, 30, 3),  -- Harvest @ Skull Valley — Ruby Tomato
    (5391, 3011311, 80, 50, 30, 3),  -- Harvest @ Skull Valley — La Noscean Lettuce
    (5392, 3011511, 80, 50, 30, 3),  -- Harvest @ Skull Valley — Marjoram
    (5393, 10005202, 80, 50, 30, 3),  -- Harvest @ Skull Valley — Moko Grass
    (5394, 10009406, 80, 30, 30, 3),  -- Harvest @ Skull Valley — Belladonna
    (5395, 10010718, 80, 50, 30, 3),  -- Harvest @ Skull Valley — All-purpose Blue Dye
    (5396, 3011310, 80, 50, 30, 3),  -- Harvest @ Bloodshore — Ruby Tomato
    (5397, 3011457, 80, 50, 30, 3),  -- Harvest @ Bloodshore — Blood Currants
    (5398, 3011506, 80, 50, 30, 3),  -- Harvest @ Bloodshore — Garlean Garlic
    (5399, 3011517, 80, 50, 30, 3),  -- Harvest @ Bloodshore — Midland Basil
    (5400, 10005202, 80, 50, 30, 3),  -- Harvest @ Bloodshore — Moko Grass
    (5401, 10010718, 80, 50, 30, 3),  -- Harvest @ Bloodshore — All-purpose Blue Dye
    (5402, 3011513, 80, 50, 30, 3),  -- Harvest @ Iron Lake — Laurel
    (5403, 10009613, 80, 50, 30, 3),  -- Harvest @ Iron Lake — Trillium
    (5404, 10009614, 80, 50, 30, 3),  -- Harvest @ Iron Lake — Trillium Bulb
    (5405, 10010718, 80, 50, 30, 3),  -- Harvest @ Iron Lake — All-purpose Blue Dye
    (5406, 3011304, 80, 40, 30, 3),  -- Harvest @ Cedarwood — Salt Leek
    (5407, 3011510, 80, 0, 30, 3),  -- Harvest @ Cedarwood — Sagolii Sage
    (5408, 10005202, 80, 50, 30, 3),  -- Harvest @ Cedarwood — Moko Grass
    (5409, 10010718, 80, 50, 30, 3),  -- Harvest @ Cedarwood — All-purpose Blue Dye
    (5410, 3011411, 80, 50, 30, 3),  -- Harvest @ Bentbranch — Chanterelle
    (5411, 3011452, 80, 50, 30, 3),  -- Harvest @ Bentbranch — Lowland Grapes
    (5412, 10005202, 80, 50, 30, 3),  -- Harvest @ Bentbranch — Moko Grass
    (5413, 10010724, 80, 50, 30, 3),  -- Harvest @ Bentbranch — All-purpose Green Dye
    (5414, 3011301, 80, 50, 30, 3),  -- Harvest @ Emerald Moss — Cieldalaes Spinach
    (5415, 3011303, 80, 50, 30, 3),  -- Harvest @ Emerald Moss — Alpine Parsnip
    (5416, 3011307, 80, 50, 30, 3),  -- Harvest @ Emerald Moss — Popoto
    (5417, 3011525, 80, 50, 30, 3),  -- Harvest @ Emerald Moss — Rye
    (5418, 10005202, 80, 50, 30, 3),  -- Harvest @ Emerald Moss — Moko Grass
    (5419, 10010724, 80, 50, 30, 3),  -- Harvest @ Emerald Moss — All-purpose Green Dye
    (5420, 3011305, 80, 50, 30, 3),  -- Harvest @ Tranquil Paths — Midland Cabbage
    (5421, 3011306, 80, 50, 30, 3),  -- Harvest @ Tranquil Paths — Wizard Eggplant
    (5422, 3011406, 80, 50, 30, 3),  -- Harvest @ Tranquil Paths — Button Mushroom
    (5423, 10005202, 80, 50, 30, 3),  -- Harvest @ Tranquil Paths — Moko Grass
    (5424, 10009611, 80, 60, 30, 3),  -- Harvest @ Tranquil Paths — Matron's Mistletoe
    (5425, 10010724, 80, 50, 30, 3),  -- Harvest @ Tranquil Paths — All-purpose Green Dye
    (5426, 3011409, 80, 50, 30, 3),  -- Harvest @ Humblehearth — White Truffle
    (5427, 10005204, 80, 50, 30, 3),  -- Harvest @ Humblehearth — Flax
    (5428, 10009401, 80, 50, 30, 3),  -- Harvest @ Humblehearth — Mandrake
    (5429, 10010724, 80, 50, 30, 3),  -- Harvest @ Humblehearth — All-purpose Green Dye
    (5430, 3011408, 80, 50, 30, 3),  -- Harvest @ Treespeak — Black Truffle
    (5431, 3011458, 80, 50, 30, 3),  -- Harvest @ Treespeak — Rolanberry
    (5432, 10005204, 80, 50, 30, 3),  -- Harvest @ Treespeak — Flax
    (5433, 10010724, 80, 50, 30, 3),  -- Harvest @ Treespeak — All-purpose Green Dye
    (5434, 3011308, 80, 40, 30, 3),  -- Harvest @ Black Brush — Wild Onion
    (5435, 3011404, 80, 60, 30, 3),  -- Harvest @ Black Brush — Cinderfoot Olive
    (5436, 3011506, 80, 50, 30, 3),  -- Harvest @ Black Brush — Garlean Garlic
    (5437, 3011508, 80, 50, 30, 3),  -- Harvest @ Black Brush — Pearl Ginger
    (5438, 10005203, 80, 50, 30, 3),  -- Harvest @ Black Brush — Cotton Boll
    (5439, 10010722, 80, 50, 30, 3),  -- Harvest @ Black Brush — All-purpose Grey Dye
    (5440, 3011313, 80, 50, 30, 3),  -- Harvest @ Drybone — Ramhorn Zucchini
    (5441, 3011314, 80, 50, 30, 3),  -- Harvest @ Drybone — Paprika
    (5442, 3011523, 80, 50, 30, 3),  -- Harvest @ Drybone — Sunset Wheat
    (5443, 10005201, 80, 50, 30, 3),  -- Harvest @ Drybone — Straw
    (5444, 10005203, 80, 50, 30, 3),  -- Harvest @ Drybone — Cotton Boll
    (5445, 10010722, 80, 50, 30, 3),  -- Harvest @ Drybone — All-purpose Grey Dye
    (5446, 3011314, 80, 50, 30, 3),  -- Harvest @ Horizon's Edge — Paprika
    (5447, 3011506, 80, 50, 30, 3),  -- Harvest @ Horizon's Edge — Garlean Garlic
    (5448, 3011508, 80, 50, 30, 3),  -- Harvest @ Horizon's Edge — Pearl Ginger
    (5449, 10005203, 80, 50, 30, 3),  -- Harvest @ Horizon's Edge — Cotton Boll
    (5450, 10009407, 80, 90, 30, 3),  -- Harvest @ Horizon's Edge — Yellow Ginseng
    (5451, 10010722, 80, 50, 30, 3),  -- Harvest @ Horizon's Edge — All-purpose Grey Dye
    (5452, 3011511, 80, 50, 30, 3),  -- Harvest @ Halatali — Marjoram
    (5453, 3011516, 80, 50, 30, 3),  -- Harvest @ Halatali — Desert Saffron
    (5454, 10005203, 80, 50, 30, 3),  -- Harvest @ Halatali — Cotton Boll
    (5455, 10010722, 80, 50, 30, 3),  -- Harvest @ Halatali — All-purpose Grey Dye
    (5456, 3011511, 80, 50, 30, 3),  -- Harvest @ Nophica's Wells — Marjoram
    (5457, 3011528, 80, 50, 30, 3),  -- Harvest @ Nophica's Wells — Almonds
    (5458, 10005203, 80, 50, 30, 3),  -- Harvest @ Nophica's Wells — Cotton Boll
    (5459, 10010722, 80, 50, 30, 3),  -- Harvest @ Nophica's Wells — All-purpose Grey Dye
    (5460, 3011317, 80, 50, 30, 3),  -- Harvest @ Dragonhead — Gysahl Greens
    (5461, 10005202, 80, 50, 30, 3),  -- Harvest @ Dragonhead — Moko Grass
    (5462, 10005204, 80, 50, 30, 3),  -- Harvest @ Dragonhead — Flax
    (5463, 10005206, 80, 50, 30, 3),  -- Harvest @ Dragonhead — Crawler Cocoon
    (5464, 3011317, 80, 50, 30, 3),  -- Harvest @ The Fields of Glory — Gysahl Greens
    (5465, 10005202, 80, 50, 30, 3),  -- Harvest @ The Fields of Glory — Moko Grass
    (5466, 10005204, 80, 50, 30, 3),  -- Harvest @ The Fields of Glory — Flax
    (5467, 3940002, 80, 90, 30, 3),  -- Spearfish @ Bearded Rock — Lugworm
    (5468, 3940004, 80, 50, 30, 3),  -- Spearfish @ Bearded Rock — Pill Bug
    (5469, 10009607, 80, 50, 30, 3),  -- Spearfish @ Bearded Rock — White Scorpion
    (5470, 10009608, 80, 50, 30, 3),  -- Spearfish @ Bearded Rock — Grass Viper
    (5471, 10010723, 80, 50, 30, 3),  -- Spearfish @ Bearded Rock — All-purpose Brown Dye
    (5472, 3011136, 80, 10, 30, 3),  -- Spearfish @ Skull Valley — Box Turtle
    (5473, 3011210, 80, 20, 30, 3),  -- Spearfish @ Skull Valley — Merlthor Goby
    (5474, 3011228, 80, 10, 30, 3),  -- Spearfish @ Skull Valley — Sea Cucumber
    (5475, 3011230, 80, 10, 30, 3),  -- Spearfish @ Skull Valley — Razor Clam
    (5476, 10010723, 80, 50, 30, 3),  -- Spearfish @ Skull Valley — All-purpose Brown Dye
    (5477, 3011136, 80, 10, 30, 3),  -- Spearfish @ Bloodshore — Box Turtle
    (5478, 10009608, 80, 50, 30, 3),  -- Spearfish @ Bloodshore — Grass Viper
    (5479, 10010723, 80, 50, 30, 3),  -- Spearfish @ Bloodshore — All-purpose Brown Dye
    (5480, 3011136, 80, 10, 30, 3),  -- Spearfish @ Iron Lake — Box Turtle
    (5481, 10009605, 80, 50, 30, 3),  -- Spearfish @ Iron Lake — Tarantula
    (5482, 10009606, 80, 50, 30, 3),  -- Spearfish @ Iron Lake — Black Scorpion
    (5483, 10010723, 80, 50, 30, 3),  -- Spearfish @ Iron Lake — All-purpose Brown Dye
    (5484, 3940003, 80, 50, 30, 3),  -- Spearfish @ Cedarwood — Moth Pupa
    (5485, 10009605, 80, 50, 30, 3),  -- Spearfish @ Cedarwood — Tarantula
    (5486, 10009607, 80, 50, 30, 3),  -- Spearfish @ Cedarwood — White Scorpion
    (5487, 10010723, 80, 50, 30, 3),  -- Spearfish @ Cedarwood — All-purpose Brown Dye
    (5488, 3011135, 80, 50, 30, 3),  -- Spearfish @ Bentbranch — Allagan Snail
    (5489, 10009608, 80, 50, 30, 3),  -- Spearfish @ Bentbranch — Grass Viper
    (5490, 10010726, 80, 50, 30, 3),  -- Spearfish @ Bentbranch — All-purpose Purple Dye
    (5491, 3011133, 80, 90, 30, 3),  -- Spearfish @ Nine Ivies — Dart Frog
    (5492, 10009605, 80, 50, 30, 3),  -- Spearfish @ Nine Ivies — Tarantula
    (5493, 10010726, 80, 50, 30, 3),  -- Spearfish @ Nine Ivies — All-purpose Purple Dye
    (5494, 3011135, 80, 50, 30, 3),  -- Spearfish @ Emerald Moss — Allagan Snail
    (5495, 3011136, 80, 10, 30, 3),  -- Spearfish @ Emerald Moss — Box Turtle
    (5496, 10010726, 80, 50, 30, 3),  -- Spearfish @ Emerald Moss — All-purpose Purple Dye
    (5497, 3011135, 80, 50, 30, 3),  -- Spearfish @ Tranquil Paths — Allagan Snail
    (5498, 3011136, 80, 10, 30, 3),  -- Spearfish @ Tranquil Paths — Box Turtle
    (5499, 10010726, 80, 50, 30, 3),  -- Spearfish @ Tranquil Paths — All-purpose Purple Dye
    (5500, 10009204, 80, 50, 30, 3),  -- Spearfish @ Humblehearth — Muddy Water
    (5501, 10009605, 80, 50, 30, 3),  -- Spearfish @ Humblehearth — Tarantula
    (5502, 10010726, 80, 50, 30, 3),  -- Spearfish @ Humblehearth — All-purpose Purple Dye
    (5503, 3011115, 80, 10, 30, 3),  -- Spearfish @ Black Brush — Striped Goby
    (5504, 3011133, 80, 90, 30, 3),  -- Spearfish @ Black Brush — Dart Frog
    (5505, 3940001, 80, 50, 30, 3),  -- Spearfish @ Black Brush — Bloodworm
    (5506, 3940003, 80, 50, 30, 3),  -- Spearfish @ Black Brush — Moth Pupa
    (5507, 10009204, 80, 50, 30, 3),  -- Spearfish @ Black Brush — Muddy Water
    (5508, 10009608, 80, 50, 30, 3),  -- Spearfish @ Black Brush — Grass Viper
    (5509, 10010721, 80, 50, 30, 3),  -- Spearfish @ Black Brush — All-purpose Black Dye
    (5510, 3011136, 80, 10, 30, 3),  -- Spearfish @ Drybone — Box Turtle
    (5511, 10009204, 80, 50, 30, 3),  -- Spearfish @ Drybone — Muddy Water
    (5512, 10009607, 80, 50, 30, 3),  -- Spearfish @ Drybone — White Scorpion
    (5513, 10009608, 80, 50, 30, 3),  -- Spearfish @ Drybone — Grass Viper
    (5514, 10010721, 80, 50, 30, 3),  -- Spearfish @ Drybone — All-purpose Black Dye
    (5515, 3011136, 80, 10, 30, 3),  -- Spearfish @ Horizon's Edge — Box Turtle
    (5516, 3940001, 80, 50, 30, 3),  -- Spearfish @ Horizon's Edge — Bloodworm
    (5517, 10009204, 80, 50, 30, 3),  -- Spearfish @ Horizon's Edge — Muddy Water
    (5518, 10010721, 80, 50, 30, 3),  -- Spearfish @ Horizon's Edge — All-purpose Black Dye
    (5519, 3011136, 80, 10, 30, 3),  -- Spearfish @ Halatali — Box Turtle
    (5520, 10009606, 80, 50, 30, 3),  -- Spearfish @ Halatali — Black Scorpion
    (5521, 10010721, 80, 50, 30, 3),  -- Spearfish @ Halatali — All-purpose Black Dye
    (5522, 3011116, 80, 20, 30, 3),  -- Spearfish @ Nophica's Wells — Sandfish
    (5523, 10009204, 80, 50, 30, 3),  -- Spearfish @ Nophica's Wells — Muddy Water
    (5524, 10010721, 80, 50, 30, 3),  -- Spearfish @ Nophica's Wells — All-purpose Black Dye
    (5525, 3011115, 80, 10, 30, 3),  -- Spearfish @ Dragonhead — Striped Goby
    (5526, 3011135, 80, 50, 30, 3),  -- Spearfish @ Dragonhead — Allagan Snail
    (5527, 10009204, 80, 50, 30, 3),  -- Spearfish @ Dragonhead — Muddy Water
    (5528, 3011115, 80, 10, 30, 3),  -- Spearfish @ The Fields of Glory — Striped Goby
    (5529, 3011135, 80, 50, 30, 3),  -- Spearfish @ The Fields of Glory — Allagan Snail
    (5530, 10009204, 80, 50, 30, 3);  -- Spearfish @ The Fields of Glory — Muddy Water

