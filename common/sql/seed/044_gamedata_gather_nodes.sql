-- Gathering node templates. One row per "harvest node" definition —
-- `harvestNodeId` keys into `DummyCommand.lua`'s aim-slot build step
-- and identifies a reusable pool of possible drops for every physical
-- spawn placed in the world.
--
-- Mirrors `harvestNodeContainer` in the prior hardcoded Lua table:
--   { grade, attempts, numItems, itemKey1..itemKeyN }
-- Flattened here into fixed columns so the row is fast to SELECT. Up
-- to 11 item keys — the DummyCommand aim slider has 11 discrete slots
-- (`+5..-5` inclusive) and each slot can be bound to at most one item.
-- Empty slots are `NULL`.

DROP TABLE IF EXISTS "gamedata_gather_nodes";
CREATE TABLE IF NOT EXISTS "gamedata_gather_nodes" (
    "id"       INTEGER PRIMARY KEY,
    "grade"    INTEGER NOT NULL DEFAULT 1,
    "attempts" INTEGER NOT NULL DEFAULT 2,
    "item1"    INTEGER DEFAULT NULL,
    "item2"    INTEGER DEFAULT NULL,
    "item3"    INTEGER DEFAULT NULL,
    "item4"    INTEGER DEFAULT NULL,
    "item5"    INTEGER DEFAULT NULL,
    "item6"    INTEGER DEFAULT NULL,
    "item7"    INTEGER DEFAULT NULL,
    "item8"    INTEGER DEFAULT NULL,
    "item9"    INTEGER DEFAULT NULL,
    "item10"   INTEGER DEFAULT NULL,
    "item11"   INTEGER DEFAULT NULL
);

-- Seed two template nodes carried forward from the prior hardcoded
-- tables so the existing DummyCommand.lua keeps behaving the same
-- after the schema-driven cut. `1001` is the tutorial copper outcrop
-- (grade 2, 2 attempts, drops Rock Salt / Bone Chip / Copper Ore);
-- `1002` is the richer grade-2 node (4 attempts, five items keyed
-- 3001..3005). These coexist with the mozk-sourced 2000-range rows
-- below; tests target the tutorial IDs so their shape is frozen.
INSERT OR IGNORE INTO "gamedata_gather_nodes"
    ("id", "grade", "attempts", "item1", "item2", "item3")
VALUES
    (1001, 2, 2, 1, 2, 3);

INSERT OR IGNORE INTO "gamedata_gather_nodes"
    ("id", "grade", "attempts", "item1", "item2", "item3", "item4", "item5")
VALUES
    (1002, 2, 4, 3005, 3003, 3002, 3001, 3004);

-- -------------------------------------------------------------------
-- Mozk-tabetai 1.x reseed. 114 rows, IDs 2000..2113 — one per
-- (retail harvest command, place) pair. Generated from mozk-raw.json
-- via the one-shot emitter in the repo history; re-run the emitter
-- against a fresh `mozk-tabetai-miner` dump and replace this block
-- verbatim to refresh.
--
-- Per-row comment format: "<command> @ <place (en)>".
-- Commands cover all six 1.x harvest actions:
--   Mine (retail 20001, internal harvest_type 22002)
--   Log  (20002 → 22003)
--   Fish (20003 → 22004)
--   Quarry    (20005 → 22005)
--   Harvest   (20006 → 22006)
--   Spearfish (20007 → 22007)
--
-- Defaults on un-sourced fields: grade=1, attempts=2. Real grade /
-- attempts values are not present in mozk's public data and would
-- need a separate source (retail dat-table dump or targeted
-- spreadsheet) to reseed accurately.
-- -------------------------------------------------------------------

INSERT OR IGNORE INTO "gamedata_gather_nodes"
    ("id", "grade", "attempts", "item1", "item2", "item3", "item4", "item5", "item6", "item7", "item8", "item9", "item10", "item11")
VALUES
    (2000, 1, 2, 5000, 5001, 5002, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Bearded Rock
    (2001, 1, 2, 5003, 5004, 5005, 5006, 5007, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Skull Valley
    (2002, 1, 2, 5008, 5009, 5010, 5011, 5012, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Bloodshore
    (2003, 1, 2, 5013, 5014, 5015, 5016, 5017, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Iron Lake
    (2004, 1, 2, 5018, 5019, 5020, 5021, 5022, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Cedarwood
    (2005, 1, 2, 5023, 5024, 5025, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Bentbranch
    (2006, 1, 2, 5026, 5027, 5028, 5029, 5030, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Emerald Moss
    (2007, 1, 2, 5031, 5032, 5033, 5034, 5035, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Tranquil Paths
    (2008, 1, 2, 5036, 5037, 5038, 5039, 5040, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Humblehearth
    (2009, 1, 2, 5041, 5042, 5043, 5044, 5045, 5046, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Treespeak
    (2010, 1, 2, 5047, 5048, 5049, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Black Brush
    (2011, 1, 2, 5050, 5051, 5052, 5053, 5054, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Drybone
    (2012, 1, 2, 5055, 5056, 5057, 5058, 5059, 5060, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Horizon's Edge
    (2013, 1, 2, 5061, 5062, 5063, 5064, 5065, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Halatali
    (2014, 1, 2, 5066, 5067, 5068, 5069, 5070, 5071, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Nophica's Wells
    (2015, 1, 2, 5072, 5073, 5074, 5075, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Nanawa Mines
    (2016, 1, 2, 5076, 5077, 5078, 5079, 5080, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ Dragonhead
    (2017, 1, 2, 5081, 5082, 5083, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Mine @ The Fields of Glory
    (2018, 1, 2, 5084, 5085, 5086, 5087, 5088, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ Bearded Rock
    (2019, 1, 2, 5089, 5090, 5091, 5092, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ Skull Valley
    (2020, 1, 2, 5093, 5094, 5095, 5096, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ Bloodshore
    (2021, 1, 2, 5097, 5098, 5099, 5100, 5101, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ Iron Lake
    (2022, 1, 2, 5102, 5103, 5104, 5105, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ Cedarwood
    (2023, 1, 2, 5106, 5107, 5108, 5109, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ Bentbranch
    (2024, 1, 2, 5110, 5111, 5112, 5113, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ Nine Ivies
    (2025, 1, 2, 5114, 5115, 5116, 5117, 5118, 5119, NULL, NULL, NULL, NULL, NULL),  -- Log @ Emerald Moss
    (2026, 1, 2, 5120, 5121, 5122, 5123, 5124, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ Tranquil Paths
    (2027, 1, 2, 5125, 5126, 5127, 5128, 5129, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ Humblehearth
    (2028, 1, 2, 5130, 5131, 5132, 5133, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ Treespeak
    (2029, 1, 2, 5134, 5135, 5136, 5137, 5138, 5139, NULL, NULL, NULL, NULL, NULL),  -- Log @ Black Brush
    (2030, 1, 2, 5140, 5141, 5142, 5143, 5144, 5145, NULL, NULL, NULL, NULL, NULL),  -- Log @ Drybone
    (2031, 1, 2, 5146, 5147, 5148, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ Horizon's Edge
    (2032, 1, 2, 5149, 5150, 5151, 5152, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ Halatali
    (2033, 1, 2, 5153, 5154, 5155, 5156, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ Nophica's Wells
    (2034, 1, 2, 5157, 5158, 5159, 5160, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ Dragonhead
    (2035, 1, 2, 5161, 5162, 5163, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Log @ The Fields of Glory
    (2036, 1, 2, 5164, 5165, 5166, 5167, 5168, 5169, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Bearded Rock
    (2037, 1, 2, 5170, 5171, 5172, 5173, 5174, 5175, 5176, 5177, 5178, NULL, NULL),  -- Fish @ Skull Valley
    (2038, 1, 2, 5179, 5180, 5181, 5182, 5183, 5184, 5185, 5186, NULL, NULL, NULL),  -- Fish @ Bald Knoll
    (2039, 1, 2, 5187, 5188, 5189, 5190, 5191, 5192, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Bloodshore
    (2040, 1, 2, 5193, 5194, 5195, 5196, 5197, NULL, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Iron Lake
    (2041, 1, 2, 5198, 5199, 5200, 5201, 5202, 5203, 5204, 5205, 5206, 5207, 5208),  -- Fish @ Cedarwood
    (2042, 1, 2, 5209, 5210, 5211, 5212, 5213, NULL, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Limsa Lominsa
    (2043, 1, 2, 5214, 5215, 5216, 5217, 5218, NULL, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Mistbeard Cove
    (2044, 1, 2, 5219, 5220, 5221, 5222, 5223, NULL, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Cassiopeia Hollow
    (2045, 1, 2, 5224, 5225, 5226, 5227, 5228, 5229, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Gridania
    (2046, 1, 2, 5230, 5231, 5232, 5233, 5234, 5235, 5236, NULL, NULL, NULL, NULL),  -- Fish @ Bentbranch
    (2047, 1, 2, 5237, 5238, 5239, 5240, 5241, 5242, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Nine Ivies
    (2048, 1, 2, 5243, 5244, 5245, 5246, 5247, 5248, 5249, 5250, NULL, NULL, NULL),  -- Fish @ Emerald Moss
    (2049, 1, 2, 5251, 5252, 5253, 5254, 5255, 5256, 5257, NULL, NULL, NULL, NULL),  -- Fish @ Tranquil Paths
    (2050, 1, 2, 5258, 5259, 5260, 5261, 5262, NULL, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Humblehearth
    (2051, 1, 2, 5263, 5264, 5265, 5266, 5267, NULL, NULL, NULL, NULL, NULL, NULL),  -- Fish @ The Mun Tuy Cellars
    (2052, 1, 2, 5268, 5269, 5270, 5271, 5272, NULL, NULL, NULL, NULL, NULL, NULL),  -- Fish @ The Tam Tara Deepcroft
    (2053, 1, 2, 5273, 5274, 5275, 5276, 5277, 5278, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Black Brush
    (2054, 1, 2, 5279, 5280, 5281, 5282, 5283, 5284, 5285, 5286, NULL, NULL, NULL),  -- Fish @ Drybone
    (2055, 1, 2, 5287, 5288, 5289, 5290, 5291, 5292, 5293, 5294, 5295, 5296, NULL),  -- Fish @ Horizon's Edge
    (2056, 1, 2, 5297, 5298, 5299, 5300, 5301, NULL, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Broken Water
    (2057, 1, 2, 5302, 5303, 5304, 5305, 5306, 5307, 5308, 5309, NULL, NULL, NULL),  -- Fish @ Halatali
    (2058, 1, 2, 5310, 5311, 5312, 5313, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Nophica's Wells
    (2059, 1, 2, 5314, 5315, 5316, 5317, 5318, NULL, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Ul'dah
    (2060, 1, 2, 5319, 5320, 5321, 5322, 5323, NULL, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Dragonhead
    (2061, 1, 2, 5324, 5325, 5326, 5327, 5328, NULL, NULL, NULL, NULL, NULL, NULL),  -- Fish @ The Fields of Glory
    (2062, 1, 2, 5329, 5330, 5331, 5332, 5333, NULL, NULL, NULL, NULL, NULL, NULL),  -- Fish @ Riversmeet
    (2063, 1, 2, 5334, 5335, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Bearded Rock
    (2064, 1, 2, 5336, 5337, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Skull Valley
    (2065, 1, 2, 5338, 5339, 5340, 5341, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Bloodshore
    (2066, 1, 2, 5342, 5343, 5344, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Iron Lake
    (2067, 1, 2, 5345, 5346, 5347, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Cedarwood
    (2068, 1, 2, 5348, 5349, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Bentbranch
    (2069, 1, 2, 5350, 5351, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Emerald Moss
    (2070, 1, 2, 5352, 5353, 5354, 5355, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Tranquil Paths
    (2071, 1, 2, 5356, 5357, 5358, 5359, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Humblehearth
    (2072, 1, 2, 5360, 5361, 5362, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Treespeak
    (2073, 1, 2, 5363, 5364, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Black Brush
    (2074, 1, 2, 5365, 5366, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Drybone
    (2075, 1, 2, 5367, 5368, 5369, 5370, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Horizon's Edge
    (2076, 1, 2, 5371, 5372, 5373, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Halatali
    (2077, 1, 2, 5374, 5375, 5376, 5377, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Nophica's Wells
    (2078, 1, 2, 5378, 5379, 5380, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ Dragonhead
    (2079, 1, 2, 5381, 5382, 5383, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Quarry @ The Fields of Glory
    (2080, 1, 2, 5384, 5385, 5386, 5387, 5388, 5389, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Bearded Rock
    (2081, 1, 2, 5390, 5391, 5392, 5393, 5394, 5395, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Skull Valley
    (2082, 1, 2, 5396, 5397, 5398, 5399, 5400, 5401, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Bloodshore
    (2083, 1, 2, 5402, 5403, 5404, 5405, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Iron Lake
    (2084, 1, 2, 5406, 5407, 5408, 5409, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Cedarwood
    (2085, 1, 2, 5410, 5411, 5412, 5413, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Bentbranch
    (2086, 1, 2, 5414, 5415, 5416, 5417, 5418, 5419, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Emerald Moss
    (2087, 1, 2, 5420, 5421, 5422, 5423, 5424, 5425, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Tranquil Paths
    (2088, 1, 2, 5426, 5427, 5428, 5429, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Humblehearth
    (2089, 1, 2, 5430, 5431, 5432, 5433, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Treespeak
    (2090, 1, 2, 5434, 5435, 5436, 5437, 5438, 5439, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Black Brush
    (2091, 1, 2, 5440, 5441, 5442, 5443, 5444, 5445, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Drybone
    (2092, 1, 2, 5446, 5447, 5448, 5449, 5450, 5451, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Horizon's Edge
    (2093, 1, 2, 5452, 5453, 5454, 5455, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Halatali
    (2094, 1, 2, 5456, 5457, 5458, 5459, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Nophica's Wells
    (2095, 1, 2, 5460, 5461, 5462, 5463, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ Dragonhead
    (2096, 1, 2, 5464, 5465, 5466, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Harvest @ The Fields of Glory
    (2097, 1, 2, 5467, 5468, 5469, 5470, 5471, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Bearded Rock
    (2098, 1, 2, 5472, 5473, 5474, 5475, 5476, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Skull Valley
    (2099, 1, 2, 5477, 5478, 5479, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Bloodshore
    (2100, 1, 2, 5480, 5481, 5482, 5483, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Iron Lake
    (2101, 1, 2, 5484, 5485, 5486, 5487, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Cedarwood
    (2102, 1, 2, 5488, 5489, 5490, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Bentbranch
    (2103, 1, 2, 5491, 5492, 5493, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Nine Ivies
    (2104, 1, 2, 5494, 5495, 5496, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Emerald Moss
    (2105, 1, 2, 5497, 5498, 5499, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Tranquil Paths
    (2106, 1, 2, 5500, 5501, 5502, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Humblehearth
    (2107, 1, 2, 5503, 5504, 5505, 5506, 5507, 5508, 5509, NULL, NULL, NULL, NULL),  -- Spearfish @ Black Brush
    (2108, 1, 2, 5510, 5511, 5512, 5513, 5514, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Drybone
    (2109, 1, 2, 5515, 5516, 5517, 5518, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Horizon's Edge
    (2110, 1, 2, 5519, 5520, 5521, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Halatali
    (2111, 1, 2, 5522, 5523, 5524, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Nophica's Wells
    (2112, 1, 2, 5525, 5526, 5527, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),  -- Spearfish @ Dragonhead
    (2113, 1, 2, 5528, 5529, 5530, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);  -- Spearfish @ The Fields of Glory

