require ("global")

--[[

Quest Script

Name:   Small Talk
Code:   DftWil
Id:     110543

Contains all default lines for talkable npcs in the Wilderness Region (aka Thanalan).
* NOTE: This quest is active for all players at all times.
]]

-- [ActorClassId] = "client_function_name"
local defaultTalkWil = {
    [1000046] = "defaultTalkWithGogofu_001",            -- Gogofu
    [1000047] = "defaultTalkWithHahayo_001",            -- Hahayo
    [1000070] = "defaultTalkWithKukumuko_001",          -- Kukumuko
    [1000293] = "defaultTalkWithDeaustie_001",          -- Deaustie     - defaultTalkWithDeaustie_002 (her dialog after unlocking WVR?)
    [1000374] = "defaultTalkWithRorojaru_001",          -- Rorojaru
    [1000597] = "defaultTalkWithNogeloix_001",          -- Nogeloix
    [1000635] = "defaultTalkWithHnaufrid_001",          -- Hnaufrid     - Will not fire since he isn't PplStd
    [1000638] = "defaultTalkWithHawazizowazi_001",      -- Hawazi Zowazi
    [1000639] = "defaultTalkWithIsabella_001",          -- Isabella
    [1000640] = "defaultTalkWithCiceroix_001",          -- Ciceroix
    [1000641] = "defaultTalkWithXaunbolo_001",          -- Xau Nbolo
    [1000642] = "defaultTalkWithOefyrblaet_001",        -- Oefyrblaet
    [1000643] = "defaultTalkWithBabaki_001",            -- Babaki
    [1000644] = "defaultTalkWithLohwaeb_001",           -- Lohwaeb
    [1000645] = "defaultTalkWithMargarete_001",         -- Margarete
    [1000646] = "defaultTalkWithRinhmaimhov_001",       -- Rinh Maimhov
    [1000647] = "defaultTalkWithLyngwaek_001",          -- Lyngwaek
    [1000648] = "defaultTalkWithWawaton_001",           -- Wawaton
    [1000649] = "defaultTalkWithDyalwann_001",          -- <<<NOT IMPLEMENTED>>> - D'yalwann - Empty function.  No wiki info, likely unused.  Has book prop.
    [1000650] = "defaultTalkWithSedemode_001",          -- <<<NOT IMPLEMENTED>>> - Sedemode  - Empty function.  No wiki info, likely unused.
    [1000651] = "defaultTalkWithPopori_001",            -- Popori
    [1000652] = "defaultTalkWithMamaza_001",            -- Mamaza
    [1000653] = "defaultTalkWithNhagiamariyo_001",      -- Nhagi Amariyo
    [1000654] = "defaultTalkWithJajanzo_001",           -- Jajanzo
    [1000655] = "defaultTalkWithJeger_001",             -- Jeger
    [1000656] = "defaultTalkWithMartine_001",           -- Martine
    [1000658] = "defaultTalkWithGairbert_001",          -- Gairbert
    [1000659] = "defaultTalkWithDrew_001",              -- Drew 
    [1000665] = "defaultTalkWithRosalind_001",          -- Rosalind
    [1000666] = "defaultTalkWithOcoco_001",             -- Ococo
    [1000668] = "defaultTalkWithUbokhn_001",            -- U'bokhn
    [1000672] = "defaultTalkWithBlandhem_001",          -- <<<NOT IMPLEMENTED>>> - Blandhem (Camp Black Brush: X:56.089 Y:199.983 Z:-462.182 rough estimate)
    [1000673] = "defaultTalkWithChechedoba_001",        -- <<<NOT IMPLEMENTED>>> - Chechedoba (Camp Black Brush: X:8.436 Y:199.973 Z:-484.073 rough estimate)
    [1000674] = "defaultTalkWithZllayan_001",           -- <<<NOT IMPLEMENTED>>> - Z'llayan (Camp Drybone)
    [1000780] = "defaultTalkWithKiora_001",             -- Kiora        - defaultTalkWithKiora_002 / 003 (informs about MRD guild)
    [1000781] = "defaultTalkWithOpondhao_001",          -- O'pondhao    - defaultTalkWithOpondhao_002 / 003 (informs about FSH guild)
    [1000782] = "defaultTalkWithBertram_001",           -- Bertram      - defaultTalkWithBertram_002 / 003 (informs about CUL guild)
    [1000783] = "defaultTalkWithMinerva_001",           -- Minvera      - defaultTalkWithMinerva_002 / 003 / 004 (informs about BSM guild. Extra dialog also if you're on a DoW/M?)
    [1000784] = "defaultTalkWithZoengterbin_001",       -- Zoengterbin  - defaultTalkWithZoengterbin_002 / 003 (informs about LNC guild)
    [1000785] = "defaultTalkWithStyrmoeya_001",         -- Styrmoeya    - defaultTalkWithStyrmoeya_002 / 003 (informs about ARC guild)
    [1000786] = "defaultTalkWithYhahamariyo_001",       -- Yhah Amariyo - defaultTalkWithYhahamariyo_002 / 003 (informs about CNJ guild)
    [1000787] = "defaultTalkWithHildie_001",            -- Hildie       - defaultTalkWithHildie_002 / 003 (informs about CRP guild)
    [1000788] = "defaultTalkWithLettice_001",           -- Lettice      - defaultTalkWithLettice_002 / 003 (informs about LTW guild)
    [1000789] = "defaultTalkWithTyon_001",              -- Tyon         - defaultTalkWithTyon_002 / 003 (informs about BTN guild)
    [1000840] = "defaultTalkWithRururaji_001",          -- Rururaji     - Presumably dialog pre-Chocobo update. Will not fire due to actor class change since then.
    [1000841] = "defaultTalkWithMomodi_001",            -- Momodi
    [1000846] = "defaultTalkWithYayake_001",            -- Yayake       - defaultTalkWithYayake_002 (her dialog after unlocking THM?)
    [1000847] = "defaultTalkWithIllofii_001",           -- I'llofii
    [1000861] = "defaultTalkWithLinette_001",           -- Linette
    [1000862] = "defaultTalkWithGagaruna_001",          -- Gagaruna
    [1000863] = "defaultTalkWithLulutsu_001",           -- Lulutsu
    [1000864] = "defaultTalkWithOtopapottopa_001",      -- Otopa Pottopa -  defaultTalkWithInn_Desk - used when Inn unlocked
    [1000865] = "defaultTalkWithThaisie_001",           -- Thaisie      - Mentions retainers, but will not fire since she's not PplStd.
    [1000887] = "defaultTalkWithZssapa_001",            -- <<<NOT IMPLEMENTED>>> - Z'ssapa  (Central Thanalan: Black Brush: 92.779999 183.837 -1030.310059) alt actor ID: 1001217 (used in a quest presumably, different outfit from wiki image)
    [1000915] = "defaultTalkWithCahernaut_001",         -- Cahernaut
    [1000916] = "defaultTalkWithAspipi_001",            -- Aspipi
    [1000917] = "defaultTalkWithGloiucen_001",          -- Gloiucen
    [1000934] = "defaultTalkWithTitinin_001",           -- Titinin
    [1000950] = "defaultTalkWithElecotte_001",          -- Elecotte
    [1000955] = "defaultTalkWithNaidazamaida_001",      -- Naida Zamaida
    [1000962] = "defaultTalkWithPapawa_001",            -- Papawa
    [1000963] = "defaultTalkWithGaleren_001",           -- Galeren
    [1000964] = "defaultTalkWithFhruybolg_001",         -- Fruhybolg
    [1000965] = "defaultTalkWithAbylgohamylgo_001",     -- Abylgo Hamylgo
    [1000966] = "defaultTalkWithFinecoromanecco_001",   -- Fineco Romanecco
    [1000967] = "defaultTalkWithSwerdahrm_001",         -- Swerdahrm
    [1000968] = "defaultTalkWithWannore_001",           -- Wannore
    [1000969] = "defaultTalkWithQmhalawi_001",          -- Q'mhalawai
    [1000994] = "defaultTalkWithLefchild_001",          -- Lefchild
    [1001007] = "defaultTalkWithHalstein_001",          -- Halstein
    [1001009] = "defaultTalkWithMelisie_001",           -- Melisie
    [1001012] = "defaultTalkWithShamanilohmani_001",    -- Shamani Lohmani
    [1001022] = "defaultTalkWithSungikelungi_001",      -- Sungi Kelungi
    [1001055] = "defaultTalkWithBouchard_001",          -- Bouchard
    [1001056] = "defaultTalkWithHolbubu_001",           -- Holbubu
    [1001073] = "defaultTalkWithObilitambili_001",      -- Obili Tambili
    [1001074] = "defaultTalkWithMiyaya_001",            -- Miyaya
    [1001075] = "defaultTalkWithBerthar_001",           -- Berthar
    [1001141] = "defaultTalkWithTutubuki_001",          -- Tutubuki
    [1001142] = "defaultTalkWithKamlitohalito_001",     -- Kamlito Halito
    [1001143] = "defaultTalkWithTotono_001",            -- Totono
    [1001144] = "defaultTalkWithFyrilsunn_001",         -- Fyrilsunn
    [1001145] = "defaultTalkWithSinette_001",           -- Sinette
    [1001146] = "defaultTalkWithZirnbyrt_001",          -- <<<NOT IMPLEMENTED>>> - Zirnbyrt - Entry Denier (East Thanalan: X:1831.565 Y:248.576 Z:448.872 Educated guess from wiki picture.  Guards unused dun01)
    [1001147] = "defaultTalkWithVhasotayuun_001",       -- <<<NOT IMPLEMENTED>>> - Vhaso Tayuun - Entry Denier (East Thanalan:  X:1818.940 Y:244.810 Z:-76.766 rough guess from vid. Guards unused dun03)
    [1001148] = "defaultTalkWithPulbeiyalbei_001",      -- <<<NOT IMPLEMENTED>>> - Pulbei Yalbei - (Entry Denier?  No wiki info)
    [1001149] = "defaultTalkWithGembert_001",           -- <<<NOT IMPLEMENTED>>> - Gembert - Entry Denier Guard (South Thanalan: X:1707.143 Y:238.150 Z:1617.570 Rough estimate. Guards unused dun06)
    [1001165] = "defaultTalkWithMumukiya_001",          -- Mumukiya
    [1001166] = "defaultTalkWithYuyubesu_001",          -- Yuyubesu
    [1001167] = "defaultTalkWithChachai_001",           -- Chachai
    [1001168] = "defaultTalkWithFifilo_001",            -- Fifilo
    [1001169] = "defaultTalkWithPierriquet_001",        -- Pierriquet
    [1001170] = "defaultTalkWithMohtfryd_001",          -- Mothfryd
    [1001171] = "defaultTalkWithQhotanbolo_001",        -- Qhota Nbolo
    [1001191] = "defaultTalkWithGuildleveClientU_001",  -- Roarich
    [1001192] = "defaultTalkWithGuildleveClientU_002",  -- Claroise
    [1001193] = "defaultTalkWithGuildleveClientU_003",  -- Uwilsyng
    [1001200] = "defaultTalkWithJannie_001",            -- Jannie
    [1001201] = "defaultTalkWithDylise_001",            -- Dylise
    [1001202] = "defaultTalkWithBarnabaix_001",         -- Barnabaix
    [1001203] = "defaultTalkWithTyagomoui_001",         -- Tyago Moui
    [1001256] = "defaultTalkWithMaginfred_001",         -- Gunnulf
    [1001257] = "defaultTalkWithOrisic_001",            -- Heibert
    [1001260] = "defaultTalkWithKlamahni_001",          -- I'paghlo
    [1001295] = "defaultTalkWithChamberlain_001",       -- <<<NOT IMPLEMENTED>>> - Chamberlain (Entry Denier?  No wiki info)
    [1001296] = "defaultTalkWithWyntkelt_001",          -- <<<NOT IMPLEMENTED>>> - Wyntkelt (Entry Denier?  No wiki info)
    [1001297] = "defaultTalkWithAudrye_001",            -- <<<NOT IMPLEMENTED>>> - Audrye (Entry Denier?  No wiki info)
    [1001314] = "defaultTalkWithFromelaut_001",         -- <<<NOT IMPLEMENTED>>> - Fromelaut (Eastern Thanalan: The Golden Bazaar)
    [1001315] = "defaultTalkWithZilili_001",            -- <<<NOT IMPLEMENTED>>> - Zilili (Eastern Thanalan: The Golden Bazaar) - Dialog doesn't match wiki, but wiki dialog isn't addressed by any function.  Changed in an update perhaps.
    [1001316] = "defaultTalkWithPapala_001",            -- <<<NOT IMPLEMENTED>>> - Papala (Eastern Thanalan: The Golden Bazaar: 1099.540039, 312.674, -1145.719971)
    [1001317] = "defaultTalkWithSasapano_001",          -- <<<NOT IMPLEMENTED>>> - Sasapano (Eastern Thanalan: The Golden Bazaar: 1134.599976, 312.193, -1128.23999)
    [1001318] = "defaultTalkWithBibiroku_001",          -- <<<NOT IMPLEMENTED>>> - Bibiroku (Eastern Thanalan: The Golden Bazaar)
    [1001319] = "defaultTalkWithBernier_001",           -- <<<NOT IMPLEMENTED>>> - Bernier (Eastern Thanalan: The Golden Bazaar)
    [1001320] = "defaultTalkWithJajaba_001",            -- <<<NOT IMPLEMENTED>>> - Jajaba (Eastern Thanalan: The Golden Bazaar)
    [1001321] = "defaultTalkWithJujuya_001",            -- <<<NOT IMPLEMENTED>>> - Jujuya (Eastern Thanalan: The Golden Bazaar)
    [1001322] = "defaultTalkWithKikinori_001",          -- <<<NOT IMPLEMENTED>>> - Kikinori (Western Thanalan: The Silver Bazaar)
    [1001323] = "defaultTalkWithCelie_001",             -- <<<NOT IMPLEMENTED>>> - Celie (Western Thanalan: The Silver Bazaar)
    [1001324] = "defaultTalkWithAgzurungzu_001",        -- <<<NOT IMPLEMENTED>>> - Agzu Rungzu (Western Thanalan: The Silver Bazaar)
    [1001325] = "defaultTalkWithDarimbeh_001",          -- <<<NOT IMPLEMENTED>>> - D'arimbeh (Western Thanalan: The Silver Bazaar)
    [1001326] = "defaultTalkWithIudprost_001",          -- <<<NOT IMPLEMENTED>>> - Iudprost (Western Thanalan: The Silver Bazaar)
    [1001327] = "defaultTalkWithTatafu_001",            -- <<<NOT IMPLEMENTED>>> - Tatafu (Western Thanalan: The Silver Bazaar)
    [1001328] = "defaultTalkWithAthalwolf_001",         -- <<<NOT IMPLEMENTED>>> - Athalwolf (Western Thanalan: The Silver Bazaar)
    [1001329] = "defaultTalkWithPadakusondaku_001",     -- <<<NOT IMPLEMENTED>>> - Padaku Sondaku (Western Thanalan: The Silver Bazaar)
    [1001330] = "defaultTalkWithBellinda_001",          -- <<<NOT IMPLEMENTED>>> - Bellinda (Eastern Thanalan, Little Ala Mhigo)
    [1001331] = "defaultTalkWithRonthfohc_001",         -- <<<NOT IMPLEMENTED>>> - Ronthfohc (Eastern Thanalan, Little Ala Mhigo)
    [1001332] = "defaultTalkWithBerahthraben_001",      -- <<<NOT IMPLEMENTED>>> - Berahthraben (Eastern Thanalan, Little Ala Mhigo)
    [1001333] = "defaultTalkWithOtho_001",              -- <<<NOT IMPLEMENTED>>> - Otho (Eastern Thanalan, Little Ala Mhigo)
    [1001334] = "defaultTalkWithRadulf_001",            -- <<<NOT IMPLEMENTED>>> - Radulf (Eastern Thanalan, Little Ala Mhigo: 1131.75, 251.29, 206.339996)
    [1001335] = "defaultTalkWithHonmeme_001",           -- <<<NOT IMPLEMENTED>>> - Honmeme (Eastern Thanalan, Little Ala Mhigo)
    [1001336] = "defaultTalkWithCatriona_001",          -- <<<NOT IMPLEMENTED>>> - Catriona (Eastern Thanalan, Little Ala Mhigo)
    [1001337] = "defaultTalkWithGrifiud_001",           -- <<<NOT IMPLEMENTED>>> - Grifiud (Eastern Thanalan, Little Ala Mhigo)
    [1001392] = "defaultTalkWithNomomo_001",            -- <<<NOT IMPLEMENTED>>> - Nomomo (Camp Black Brush: X:24.274 Y:200.003 Z:-473.548 rough estimate) - If arg1=true, says different dialog.
    [1001415] = "defaultTalkWithAnthoinette_001",       -- Anthoinette
    [1001416] = "defaultTalkWithWisemoon_001",          -- Wise Moon
    [1001417] = "defaultTalkWithApachonaccho_001",      -- Apacho Naccho
    [1001418] = "defaultTalkWithWyznguld_001",          -- Wyznguld
    [1001419] = "defaultTalkWithNeymumu_001",           -- Neymumu
    [1001420] = "defaultTalkWithSafufu_001",            -- Safufu
    [1001421] = "defaultTalkWithPenelizuneli_001",      -- Peneli Zuneli
    [1001422] = "defaultTalkWithMilgogo_001",           -- Milgogo
    [1001423] = "defaultTalkWithMumutano_001",          -- Mumutano
    [1001424] = "defaultTalkWithGegeissa_001",          -- Gegeissa
    [1001425] = "defaultTalkWithGdatnan_001",           -- G'datnan 
    [1001426] = "defaultTalkWithHehena_001",            -- Hehena
    [1001427] = "defaultTalkWithGuillaunaux_001",       -- Guillaunaux
    [1001428] = "defaultTalkWithYuyuhase_001",          -- Yuyuhase
    [1001429] = "defaultTalkWithLulumo_001",            -- Lulumo
    [1001438] = "defaultTalkWithNokksushanksu_001",     -- Nokksu Shanksu
    [1001439] = "defaultTalkWithThimm_001",             -- Thimm
    [1001440] = "defaultTalkWithQaruru_001",            -- Qaruru
    [1001441] = "defaultTalkWithWracwulf_001",          -- Wracwulf
    [1001442] = "defaultTalkWithWenefreda_001",         -- Wenefreda
    [1001443] = "defaultTalkWithJudithe_001",           -- Judithe
    [1001444] = "defaultTalkWithRobyn_001",             -- Robyn
    [1001445] = "defaultTalkWithSingleton_001",         -- Singleton
    [1001446] = "defaultTalkWithFiachre_001",           -- <<<NOT IMPLEMENTED>>> - Fiachre (Western Thanalan Ferry Docks)
    [1001447] = "defaultTalkWithTaylor_001",            -- <<<NOT IMPLEMENTED>>> - Taylor (Western Thanalan Ferry Docks: -2195.070068, 14.495, -417.200012)
    [1001448] = "defaultTalkWithWalhbert_001",          -- <<<NOT IMPLEMENTED>>> - Walhbert (Western Thanalan Ferry Docks)
    [1001449] = "defaultTalkWithSpiralingpath_001",     -- <<<NOT IMPLEMENTED>>> - Spiraling Path (Western Thanalan Ferry Docks)
    [1001450] = "defaultTalkWithSasapiku_001",          -- <<<NOT IMPLEMENTED>>> - Sasapiku (Western Thanalan Ferry Docks)
    [1001451] = "defaultTalkWithDoll001_001",           -- Mammet (Eshtaime's Lapidaries [GSM])
    [1001452] = "defaultTalkWithDoll002_001",           -- Mammet (Eshtaime's Lapidaries [GSM] #2)
    [1001453] = "defaultTalkWithDoll003_001",           -- Mammet (Sunsilk Tapestries [WVR])
    [1001454] = "defaultTalkWithDoll004_001",           -- Mammet (Frondale's Phrontistery [ALC])
    [1001455] = "defaultTalkWithDoll005_001",           -- Mammet (Merchant Strip)
    [1001462] = "defaultTalkWithQatanelhah_001",        -- Qata Nelhah
    [1001463] = "defaultTalkWithKukusi_001",            -- Kukusi
    [1001464] = "defaultTalkWithVannes_001",            -- Vannes
    [1001465] = "defaultTalkWithTatasha_001",           -- Tatasha
    [1001466] = "defaultTalkWithXdhilogo_001",          -- X'dhilogo
    -- [1001467] = "",                                  -- Dariustel    - No dialog. Supposed to be flagged as untargetable
    -- [1001468] = "",                                  -- Guencen      - No dialog. Supposed to be flagged as untargetable
    [1001475] = "defaultTalkWithDiriaine_001",          -- Diriaine
    [1001476] = "defaultTalkWithCrhabye_001",           -- C'rhabye
    [1001471] = "downTownTalk",                         -- Kokobi
    [1001472] = "defaultTalkWithMimishu_001",           -- Mimishu
    [1001503] = "defaultTalkWithGerland_001",           -- <<<NOT IMPLEMENTED>>> - Gerland (Western Thanalan Ferry Docks)
    [1001565] = "defaultTalkWithEleanor_001",           -- Eleanor
    [1001596] = "defaultTalkWithAbelard_001",           -- <<<NOT IMPLEMENTED>>> - Abelard (Western Thanalan: The Coffer & Coffin: -1726, 56.625, -317
    [1001597] = "defaultTalkWithHaipoeipo_001",         -- <<<NOT IMPLEMENTED>>> - Haipo Eipo (Western Thanalan: The Coffer & Coffin)
    [1001598] = "defaultTalkWithBartholomew_001",       -- <<<NOT IMPLEMENTED>>> - Bartholomew (Western Thanalan: The Coffer & Coffin)
    [1001599] = "defaultTalkWithKokofubu_001",          -- <<<NOT IMPLEMENTED>>> - Kokofubu (Eastern Thanalan: Mythril Pit T-8)
    [1001600] = "defaultTalkWithBertouaint_001",        -- <<<NOT IMPLEMENTED>>> - Bertouaint (Eastern Thanalan: Mythril Pit T-8)
    [1001601] = "defaultTalkWithAldebrand_001",         -- <<<NOT IMPLEMENTED>>> - Aldebrand (Eastern Thanalan: Mythril Pit T-8)
    [1001602] = "defaultTalkWithPyhajawantal_001",      -- <<<NOT IMPLEMENTED>>> - Pyha Jawantal (Eastern Thanalan: Mythril Pit T-8)
    --[1001624] = "talkIdayCap",                        -- <<<NOT IMPLEMENTED>>> - Flame Lieutenant Somber Meadow   (Foundation Day 2011 Dialog) - OLD EVENT NPC: Replaced by 2012 version
    --[1001625] = "talkIday1",                          -- <<<NOT IMPLEMENTED>>> - Flame Sergeant Mimio Mio         (Foundation Day 2011 Dialog) - OLD EVENT NPC: Replaced by 2012 version
    --[1001626] = "talkIday2",                          -- <<<NOT IMPLEMENTED>>> - Flame Private Sisimuza Tetemuza  (Foundation Day 2011 Dialog) - OLD EVENT NPC: Replaced by 2012 version
    [1001630] = "defaultTalkWithChocobo_001",           -- <<<NOT IMPLEMENTED>>> - Chocobo (Western Thanalan: The Coffer & Coffin) - Stands beside Haipo Eipo
    [1001685] = "defaultTalkWithAdalbert_001",          -- <<<NOT IMPLEMENTED>>> - Flame Sergeant Cotter (Ul'dah: Merchant Strip: -0.92 196.100 126.32) - Double check caps.
    [1001699] = "defaultTalkWithJandonaut_001",         -- <<<NOT IMPLEMENTED>>> - Flame Sergeant Fouillel (Southern Thanalan: Camp Broken Water: 1704 296.001 999)
    [1001712] = "defaultTalkWithGuillestet_001",        -- Guillestet
    [1001713] = "defaultTalkWithHCidjaa_001",           -- H'cidjaa
    [1001714] = "defaultTalkWithAutgar_001",            -- <<<NOT IMPLEMENTED>>> - Autgar (Ul'dah: Airship Landing)
    [1001715] = "defaultTalkWithAhldbyrt_001",          -- <<<NOT IMPLEMENTED>>> - Ahldbyrt (Ul'dah: Airship Landing)
    [1001716] = "defaultTalkWithNeymiFunomi_001",       -- <<<NOT IMPLEMENTED>>> - Neymi Funomi (Ul'dah: Airship Landing)
    [1001717] = "defaultTalkWithGoodife_001",           -- Goodife
    [1001726] = "defaultTalkWithAistan_001",            -- Aistan
    [1001727] = "defaultTalkWithMateria_001",           -- <<<NOT IMPLEMENTED>>> - Mutamix Bubblypots (Central Thanalan: 243.858002, 247.8, -1030.136963) - Check feet clipping @ pos
    [1001728] = "defaultTalkWithSWYNBROES_001",         -- <<<NOT IMPLEMENTED>>> - Swynbroes (Central Thanalan: 242.641006, 247.6, -1024.494019)
    [1001729] = "defaultTalkWithKokosamu_001",          -- <<<NOT IMPLEMENTED>>> - Kokosamu (Central Thanalan: 255.651001, 248.5, -1030.152954) - Check feet clipping @ pos
    [1001730] = "defaultTalkWithF_HOBHAS_001",          -- <<<NOT IMPLEMENTED>>> - F'hobhas (Central Thanalan: 258.665009, 248, -1021.666992)
    [1001753] = "defaultTalkWithPelhiEpocan_001",       -- <<<NOT IMPLEMENTED>>> - Pelhi Epocan (Ul'dah: Airship Landing)
    [1001754] = "defaultTalkWithViolenne_001",          -- <<<NOT IMPLEMENTED>>> - Violenne (Ul'dah: Airship Landing)
    [1001770] = "defaultTalkWithEara_001",              -- Eara
    [1001771] = "defaultTalkWithLiaime_001",            -- Liaime
    [1001834] = "defaultTalkWithLUDOLD_001",            -- <<<NOT IMPLEMENTED>>> - Flame Commander Ashdale - (Eastern Thanalan: 1410, 256, 187)
    [1001840] = "defaultTalkWithPAHJAZHWAN_001",        -- <<<NOT IMPLEMENTED>>> - Pahja Zhwan - (Ul'dah: Miner's Guild: -113.190002 194.2 324.25) - Double check caps.
    [1001894] = "defaultTalkWithDonner_001",            -- <<<NOT IMPLEMENTED>>> - Flame Private Greave - (North Thanalan: Bluefog)
    [1001911] = "defaultTalkWithLolomaya_001",          -- <<<NOT IMPLEMENTED>>> - Lolomaya - (North Thanalan: Camp Bluefog) Has unused argument. Dialog doesn't match wiki, but dftwil doesn't call it anywhere.  Update change perhaps.
    [1001925] = "defaultTalkWithHortwann_001",          -- <<<NOT IMPLEMENTED>>> - Flame Private Hanskalsyn - (North Thanalan: Camp Bluefog)
    [1001932] = "defaultTalkWithSIBOLD_001",            -- Sibold
    [1001953] = "defaultTalkWithBerndan_001",           -- Berndan
    [1002047] = "defaultTalkWithKopuruFupuru_001",      -- Kopuru Fupuru - Inn NPC -  defaultTalkWithInn_Desk_2 used when Inn unlocked
    [1002101] = "defaultTalkWithDuraltharal_001",       -- Dural Tharal
    [1002110] = "processEventSOMBER",                   -- Flame Lieutenant Somber Meadow   (Foundation Day 2012 Dialog) Spl000 staticactor
    [1002111] = "processEventMIMIO",                    -- Flame Sergeant Mimio Mio         (Foundation Day 2012 Dialog) Spl000 staticactor
    [1002112] = "processEventSISIMUZA",                 -- Flame Private Sisimuza Tetemuza  (Foundation Day 2012 Dialog) Spl000 staticactor
    [1002116] = "defaultTalkWithHAVAK_ALVAK_001",       -- <<<NOT IMPLEMENTED>>> - Havak Alvak (Ul'dah: Milvaneth Sacrarium)
    [1060028] = "defaultTalkWithCURIOUS_001",           -- <<<NOT IMPLEMENTED>>> - Curious Gorge (Western Thanalan: -1116.040039, 53.2, 285.48999)? - defaultTalkWithCURIOUS_002
    [1060029] = "defaultTalkWithSarra_001",             -- <<<NOT IMPLEMENTED>>> - Sarra (Location unknown) defaultTalkWithSarra_002 / 003
    [1060032] = "defaultTalkWithWidargeli_001",         -- <<<NOT IMPLEMENTED>>> - Widargelt (Eastern Thanalan: Little Ala Mhigo: 1213.670044, 251.439, 107.290001) - defaultTalkWithWidargeli_002
    [1060033] = "defaultTalkWithErik_001",              -- <<<NOT IMPLEMENTED>>> - Erik  (Ul'dah: -32.75 192.1 45.810001) - defaultTalkWithErik_002
    [1060035] = "defaultTalkWithLalai_001",             -- <<<NOT IMPLEMENTED>>> - Lalai (Ul'dah: Milvaneth Sacrarium: 18.16, 206, 283.670013)        defaultTalkWithLalai_002 / 003 / 004 / 005 / 006 / 007 / 101
    [1060036] = "defaultTalkWithKazagg_001",            -- <<<NOT IMPLEMENTED>>> - Kazagg Chah (Western Thalanan: -1506.540039, 10.241, -233.970001)  defaultTalkWithKazagg_002 / 003 / 004 / 005 / 006 / 007
    [1060037] = "defaultTalkWithHateli_001",            -- <<<NOT IMPLEMENTED>>> - Dozol Meloc (Western Thanalan: -1513.660034, 10.617, -235.220001)  defaultTalkWithHateli_002 / 003 / 004 / 005 / 006 / 007
    [1060038] = "defaultTalkWithDaza_001",              -- <<<NOT IMPLEMENTED>>> - 269th Order Mendicant Da Za (Western Thanalan: Somewhere in the cave around -1567, 25, -170) -  defaultTalkWithDaza_002 / 003 / 004 / 005 / 006 / 007
    [1060042] = "defaultTalkWithJenlyns_001",           -- Jenlyns      - defaultTalkWithJenlyns_002 (PLD-unlocked specific dialog?)
    [1200120] = "bookTalk",                             -- Dusty Tomes  - Will not fire since it isn't PplStd.  Identical dialog regardless.
    [1500059] = "defaultTalkWithLdhakya_001",           -- <<<NOT IMPLEMENTED>>> - L'dhakya (Western Thanalan Ferry Docks)
    [1500109] = "defaultTalkWithSylviel_001",           -- <<<NOT IMPLEMENTED>>> - Sylviel (Western Thanalan Ferry Docks)
    [1500110] = "defaultTalkWithSamigamduhla_001",      -- <<<NOT IMPLEMENTED>>> - Sami Gamduhla (Western Thanalan Ferry Docks)
    [1500126] = "tribeTalk",                            -- Vavaki
    [1500129] = "defaultTalkWithYayatoki_001",          -- Yayatoki
    [1500230] = "defaultTalkCaravanChocoboUld_001",     -- Pack Chocobo (needs verifying)
    [1700039] = "defaultTalkWithBATERICH_100",          -- Baterich
        
    --[1090549] = "defaultTalkWithInn_ExitDoor"         -- Ul'dah Inn Exit Door pushEvent - "Leave your room?"
    --[1200336] = "defaultTalkWithInn_ExitDoor"         -- Ul'dah Inn Exit Door talkEvent - "Leave your room?"
    
--[[ Need sourcing
    [???] = "defaultTalkCaravanChocoboUld_001"           -- Presumably used on the Caravan Chocobo escorts? Does a little animation.
    [???] = "defaultTalkWithExit01"                      -- "Leave this place?" - For quest locations like that four-bedroom room perhaps?
    [???] = "defaultTalkWithMarketNpc"                   -- NPC in the middle of the market wards that lets you port around I'm guessing?
    [???] = "defaultTalkWithHamletGuardUld_001"          -- 
--]]
    

    
    }
 
    
function onTalk(player, quest, npc, eventName)

    local npcId = npc:GetActorClassId();
    local clientFunc = defaultTalkWil[npcId];
    
    if (npcId == 1000864) then -- Otopa Pottopa (Adv. Guild Inn NPC)
        if (player:IsQuestCompleted(110848)) then -- "Ring of Deceit" completed.
            callClientFunction(player, "delegateEvent", player, quest, "defaultTalkWithInn_Desk");
        else
            callClientFunction(player, "delegateEvent", player, quest, clientFunc);
        end
    elseif (npcId == 1002047) then -- Kopuru Fupuru (Rear-Entrance Inn NPC)
        if (player:IsQuestCompleted(110848)) then -- "Ring of Deceit" completed.
            defaultTalkWithInn(player, quest, "defaultTalkWithInn_Desk_2");
        else
            callClientFunction(player, "delegateEvent", player, quest, clientFunc);
        end
    elseif ((npcId >= 1002110) and (npcId <= 1002112)) then  -- Foundation Day 2012 NPCs
        talkWithSpecial(player, npcId, clientFunc)
    else
        callClientFunction(player, "delegateEvent", player, quest, clientFunc);
    end
    
    player:EndEvent();
end


function IsQuestENPC(player, quest, npc)
    return defaultTalkWil[npc:GetActorClassId()] ~= nil;
end



function defaultTalkWithInn(player, quest, clientFunc)
    local choice = callClientFunction(player, "delegateEvent", player, quest, clientFunc);
        
    if (choice == 1) then
        GetWorldManager():DoZoneChange(player, 244, nil, 0, 15, 0.048, 0, -5.737, 0);
    elseif (choice == 2) then
        if (player:GetHomePointInn() ~= 3) then
            player:SetHomePointInn(3);
            player:SendGameMessage(GetWorldMaster(), 60019, 0x20, 3071); --Secondary homepoint set to the Hourglass
        else            
            player:SendGameMessage(GetWorldMaster(), 51140, 0x20); --This inn is already your Secondary Homepoint
        end
    end
end



function talkWithSpecial(player, npcId, clientFunc)
        local splQuest = GetStaticActor("Spl000");
        local magickedPrism = 0;
        callClientFunction(player, "delegateEvent", player, splQuest, clientFunc, magickedPrism);
end