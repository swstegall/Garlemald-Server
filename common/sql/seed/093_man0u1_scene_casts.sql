-- Man0u1 "Court in the Sands" — scene-cast eventnpc spawns
-- (Garlemald-Server #53).
--
-- Seeds every quest-only NPC station man0u1.lua addresses that has no
-- upstream spawn row: the Amajina & Sons miner-scene cast, the Coliseum
-- lobby/echo cast, and the Phrontistery ward cast — all in the ONE
-- Ul'dah-half private area the client provably loads (209,
-- 'PrivateAreaMasterPast', 5 — pmeteor's pgl200 echo area; the three
-- scene groups share it at disjoint coordinates, SetENpc arming keeps
-- them sequence-scoped) — plus the public zone-209 and zone-170
-- additions (Yoyobina, the Concern-stage/gate-muster F'lhaminn,
-- Faustigeant, the Camp Black Brush echo cast).
--
-- Coordinate evidence: the client quest_marker.csv 110010xx block's x/z
-- pairs are literal world coordinates (verified: 11001009 == Linette's
-- public spawn row, 11001018 == Nogeloix's); floor y values come from
-- the neighbouring seed-031/059 rows (Amajina interior 195.6, coliseum
-- lobby 193.2-195.05, Phrontistery ward 229.5, camp 201.0). Rows
-- without a marker are clustered plausibly around the scene anchor and
-- tagged "clustered" — rotations are eyeballed toward the scene centre
-- and all such rows are flagged for in-client tuning.
--
-- (1) classPath fills: most of this cast ships STRIPPED in gamedata
-- (classPath '' / propertyFlags 0 / null eventConditions) in BOTH
-- upstreams — an actor spawned from a classPath-less class hard-crashes
-- the client (the seed/061 Wine-crash family, populace flavour;
-- seed/076 is the identical fix for the man0g1 CNJ cast). Fill = the
-- standard populace recipe: PopulaceStandard + propertyFlags 19 +
-- talkDefault/noticeEvent. displayNameIds already correct in seed/003 —
-- untouched. Guarded so a future upstream refresh with real rows wins.
-- (1000842/1001283/1001284 get their emote-enabled condition superset
-- in seed/095 — the fill here is the crash-safety baseline.)
--
-- Spawn ids 2400-2441: explicit clean block above the highest explicit
-- seed id (2264, seed/059) AND the ~57 AUTOINCREMENT rows seeds
-- 056-087 add (≈2265-2321). uniqueId prefix 'man0u1_'.
--
-- Idempotent: UPDATEs guarded, INSERTs OR IGNORE on id. No schema change.

-- ---- (1) classPath fills for the stripped cast (seed/076 recipe) -----

UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Populace/PopulaceStandard',
    "propertyFlags" = 19,
    "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}'
WHERE "id" IN (
    1000842, -- F'lhaminn (scene/teach variant; emote superset in seed/095)
    1001054, -- Corguevais (beaten-in-the-mines scene variant)
    1001283, -- Manic Miner (emote superset in seed/095)
    1001284, -- Maddened Miner (emote superset in seed/095)
    1001286, -- Nittma Guttma
    1001287, -- Maudlin Miner
    1001288, -- Mocking Miner
    1001289, -- Monitoring Miner
    1001290, -- Displeased Dancer
    1000690, -- Muscular Miner
    1000981, -- Close-fisted Woman
    1000895, -- Astonished Adventurer
    1000857, -- Popokkuli
    1000858, -- Seserukka
    1000039, -- Greinfarr
    1000037, -- Niellefresne (base class; GLD echo scene)
    1000699, -- Qualmish Miner
    1001207, -- Worrisome Assistant
    1001210, -- Well-washed Leech
    1000852, -- Master Faustigeant
    1000927, -- Yoyobina
    1000038, -- F'lhaminn (base populace; gate muster + Concern stage)
    1001515, -- Ascilia (camp-scene variant)
    1000043, -- Corguevais (present-day; camp echo)
    1000808, -- Untidy Outlander
    1000809, -- Malnourished Midlander
    1000817  -- Helpless Hyur (the camp leader)
) AND "classPath" = '';

-- Thancred's camp class already carries talk/notice conditions in
-- seed/003 — restore classPath/flags ONLY (nothing else clobbered).
UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Populace/PopulaceStandard',
    "propertyFlags" = 19
WHERE "id" = 1000185 AND "classPath" = '';

-- Slow-witted Miner has a classPath but flags 0 / null conditions —
-- the ward crowd needs him talkable (processEvent210_6).
UPDATE "gamedata_actor_class" SET
    "propertyFlags" = 19,
    "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}'
WHERE "id" = 1000689 AND "eventConditions" IS NULL;

-- ---- (2) Miner scene — Amajina & Sons interior (PA 209/5, y=195.6) ---

-- Anchored rows: markers 11001009/10/11/12/13; Corguevais lies beaten
-- on the floor between the miners (retail CS8 — animationId 1000, the
-- seed/059 collapsed-corpse motion; flag for live check). Everyone else
-- clustered (-95..-110, 310..328) facing the scene centre.
INSERT OR IGNORE INTO "server_spawn_locations"
    ("id", "actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    (2400, 1000861, 'man0u1_linette_scene',       209, 'PrivateAreaMasterPast', 5,  -92.38, 195.6, 313.43, -0.97, 0, 0, null),   -- marker 11001009
    (2401, 1000842, 'man0u1_flhaminn_teach',      209, 'PrivateAreaMasterPast', 5,  -96.68, 195.6, 313.17, -0.66, 0, 0, null),   -- marker 11001010
    (2402, 1001054, 'man0u1_corguevais_scene',    209, 'PrivateAreaMasterPast', 5, -106.50, 195.6, 320.00,  1.57, 0, 1000, null), -- between the miners (clustered; collapsed pose)
    (2403, 1001283, 'man0u1_manic_miner',         209, 'PrivateAreaMasterPast', 5, -108.41, 195.6, 326.53,  2.36, 0, 0, null),   -- marker 11001011
    (2404, 1001284, 'man0u1_maddened_miner',      209, 'PrivateAreaMasterPast', 5, -107.48, 195.6, 324.55,  2.27, 0, 0, null),   -- marker 11001012
    (2405, 1001286, 'man0u1_nittma_guttma',       209, 'PrivateAreaMasterPast', 5, -104.00, 195.6, 318.00,  0.79, 0, 0, null),   -- near marker 11001013
    (2406, 1001287, 'man0u1_maudlin_miner',       209, 'PrivateAreaMasterPast', 5, -109.50, 195.6, 322.00,  1.20, 0, 0, null),   -- clustered
    (2407, 1001288, 'man0u1_mocking_miner',       209, 'PrivateAreaMasterPast', 5, -110.00, 195.6, 318.50,  1.50, 0, 0, null),   -- clustered
    (2408, 1001289, 'man0u1_monitoring_miner',    209, 'PrivateAreaMasterPast', 5, -108.50, 195.6, 315.00,  1.90, 0, 0, null),   -- clustered
    (2409, 1001290, 'man0u1_displeased_dancer',   209, 'PrivateAreaMasterPast', 5, -102.50, 195.6, 322.50, -0.60, 0, 0, null),   -- clustered
    (2410, 1000690, 'man0u1_muscular_miner',      209, 'PrivateAreaMasterPast', 5, -105.50, 195.6, 327.50,  0.40, 0, 0, null),   -- clustered
    (2411, 1000981, 'man0u1_closefisted_woman',   209, 'PrivateAreaMasterPast', 5,  -98.50, 195.6, 316.50, -1.30, 0, 0, null),   -- clustered
    (2412, 1000895, 'man0u1_astonished_adv',      209, 'PrivateAreaMasterPast', 5,  -97.50, 195.6, 319.50, -1.10, 0, 0, null),   -- clustered
    (2413, 1001203, 'man0u1_tyago_moui',          209, 'PrivateAreaMasterPast', 5, -100.50, 195.6, 312.00, -2.60, 0, 0, null),   -- clustered
    (2414, 1000637, 'man0u1_shilgen',             209, 'PrivateAreaMasterPast', 5,  -95.50, 195.6, 310.50, -2.90, 0, 0, null),   -- clustered
    (2415, 1600042, 'man0u1_nortmoen',            209, 'PrivateAreaMasterPast', 5,  -94.00, 195.6, 312.50, -2.30, 0, 0, null),   -- clustered
    (2416, 1000857, 'man0u1_popokkuli',           209, 'PrivateAreaMasterPast', 5,  -99.00, 195.6, 324.00, -0.30, 0, 0, null),   -- clustered
    (2417, 1000858, 'man0u1_seserukka',           209, 'PrivateAreaMasterPast', 5,  -98.00, 195.6, 325.50, -0.45, 0, 0, null);   -- clustered

-- ---- (3) Coliseum lobby + post-match echo (PA 209/5) -----------------

-- Lulutsu copies his public seed-031 spot (row 178); Papawa's stand is
-- explicit; the crowd is clustered (-180..-190, 193..195, 190..210).
-- Greinfarr + Niellefresne carry the man0u140 gathering echo.
INSERT OR IGNORE INTO "server_spawn_locations"
    ("id", "actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    (2418, 1000863, 'man0u1_lulutsu_lobby',       209, 'PrivateAreaMasterPast', 5, -193.46, 195.05, 183.65,  0.61, 0, 0, null),  -- copy of public row 178
    (2419, 1000962, 'man0u1_papawa_lobby',        209, 'PrivateAreaMasterPast', 5, -175.05, 193.20, 201.17, -2.10, 0, 0, null),
    (2420, 1000965, 'man0u1_abylgo_hamylgo',      209, 'PrivateAreaMasterPast', 5, -183.00, 194.00, 196.00,  0.80, 0, 0, null),  -- clustered
    (2421, 1000964, 'man0u1_fruhybolg',           209, 'PrivateAreaMasterPast', 5, -186.50, 194.50, 203.00, -1.20, 0, 0, null),  -- clustered
    (2422, 1000967, 'man0u1_swerdahrm',           209, 'PrivateAreaMasterPast', 5, -181.00, 193.50, 207.50,  2.80, 0, 0, null),  -- clustered
    (2423, 1000039, 'man0u1_greinfarr_echo',      209, 'PrivateAreaMasterPast', 5, -185.50, 195.00, 193.00, -0.64, 0, 0, null),
    (2424, 1000037, 'man0u1_niellefresne_echo',   209, 'PrivateAreaMasterPast', 5, -187.00, 195.00, 195.00,  2.50, 0, 0, null);

-- ---- (4) Phrontistery sickroom ward (PA 209/5, y=229.5) --------------

-- Ward crowd clustered (-210..-224, 300..315); Faustigeant explicit;
-- the ward door is the PA copy of class 1090119's public row 250 (the
-- SEQ_100/105 exit push — seed/094 gives the class its push condition).
INSERT OR IGNORE INTO "server_spawn_locations"
    ("id", "actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    (2425, 1000689, 'man0u1_slowwitted_miner',    209, 'PrivateAreaMasterPast', 5, -212.50, 229.5, 303.50,  1.20, 0, 0, null),  -- clustered
    (2426, 1000690, 'man0u1_muscular_miner_ward', 209, 'PrivateAreaMasterPast', 5, -214.50, 229.5, 306.00,  0.90, 0, 0, null),  -- clustered
    (2427, 1000699, 'man0u1_qualmish_miner',      209, 'PrivateAreaMasterPast', 5, -218.00, 229.5, 309.50,  0.60, 0, 0, null),  -- clustered
    (2428, 1001207, 'man0u1_worrisome_assistant', 209, 'PrivateAreaMasterPast', 5, -221.00, 229.5, 311.50,  0.30, 0, 0, null),  -- clustered
    (2429, 1001210, 'man0u1_wellwashed_leech',    209, 'PrivateAreaMasterPast', 5, -223.00, 229.5, 313.00,  0.10, 0, 0, null),  -- clustered
    (2430, 1000852, 'man0u1_faustigeant_ward',    209, 'PrivateAreaMasterPast', 5, -220.00, 229.5, 308.00,  2.60, 0, 0, null),
    (2431, 1090119, 'man0u1_ward_door_pa',        209, 'PrivateAreaMasterPast', 5, -216.38, 229.5, 302.07,  0.00, 0, 0, null); -- PA copy of public row 250

-- ---- (5) PUBLIC zone 209 additions -----------------------------------

INSERT OR IGNORE INTO "server_spawn_locations"
    ("id", "actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    -- Yoyobina at the coliseum entrance (marker 11001006 — the exact
    -- spot of the public 1090283 row 257; rotation copied from it,
    -- facing out toward the walk-up).
    (2432, 1000927, 'man0u1_yoyobina',            209, '', 0, -185.26, 195.00, 179.74, -2.28, 0, 0, null),
    -- F'lhaminn at the Concern stage (marker 11001017 — the SEQ_075
    -- escort-payment talk).
    (2433, 1000038, 'man0u1_flhaminn_stage',      209, '', 0,  -91.35, 196.02, 323.18, -0.50, 0, 0, null),
    -- Faustigeant near the ward door, public floor (the SEQ_085 talk;
    -- clustered off marker 11001019's door at -216.38/302.07).
    (2434, 1000852, 'man0u1_faustigeant_public',  209, '', 0, -214.00, 229.60, 296.00, -0.37, 0, 0, null);

-- ---- (6) PUBLIC zone 170 additions -----------------------------------

INSERT OR IGNORE INTO "server_spawn_locations"
    ("id", "actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    -- F'lhaminn at the Gate of Nald muster (marker 11001014; y=183.0
    -- interpolated from the nearby seed-031 zone-170 rows — FLAG FOR
    -- IN-CLIENT TUNING).
    (2435, 1000038, 'man0u1_flhaminn_gate',       170, '', 0, -34.50, 183.0, -68.90,  0.00, 0, 0, null),
    -- Camp Black Brush echo cast (public — design-doc checkpoint 4;
    -- camp floor y=201.0 from the seed-031 camp anchors). Ascilia at
    -- marker 11001015, the camp leader at 11001016; the rest clustered.
    (2436, 1001515, 'man0u1_ascilia_camp',        170, '', 0,  41.10, 201.0, -480.00, 1.68, 0, 0, null),  -- marker 11001015
    (2437, 1000043, 'man0u1_corguevais_camp',     170, '', 0,  42.50, 201.0, -481.50, 1.40, 0, 0, null),  -- clustered
    (2438, 1000185, 'man0u1_thancred_camp',       170, '', 0,  44.00, 201.0, -479.00, 1.20, 0, 0, null),  -- clustered
    (2439, 1000808, 'man0u1_untidy_outlander',    170, '', 0,  39.50, 201.0, -478.50, 2.20, 0, 0, null),  -- clustered
    (2440, 1000809, 'man0u1_malnourished_mid',    170, '', 0,  40.50, 201.0, -482.50, 1.00, 0, 0, null),  -- clustered
    (2441, 1000817, 'man0u1_helpless_hyur',       170, '', 0,  50.43, 201.0, -480.99, -1.46, 0, 0, null); -- marker 11001016
