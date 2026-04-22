require ("global")

--[[

Quest Script

Name:   Small Talk
Code:   DftFst
Id:     110542

Contains all default lines for talkable npcs in the Forest Region (aka Black Shroud).
* NOTE: This quest is active for all players at all times.
]]

-- [ActorClassId] = "client_function_name"
local defaultTalkFst = {
    [1000066] = "defaultTalkWithAlixe_001",                 -- Alixe                (Gridania: LTW Guild)
    [1000067] = "defaultTalkWithDadalo_001",                -- Dadalo               (Gridania: LTW Guild)
    [1000068] = "defaultTalkWithKain_001",                  -- Kain                 (Gridania: LTW Guild)
    [1000069] = "defaultTalkWithJolline_001",               -- Jolline              (Gridania: LNC Guild)
    [1000071] = "defaultTalkWithBertennant_001",            -- Bertennant           (Gridania: Blue Badger Gate)
    [1000072] = "defaultTalkWithMitainie_001",              -- Mitainie 			(Gridania: White Wolf Gate)
    [1000074] = "defaultTalkWithOnguen_001",                -- Onguen               (Gridania: BTN Guild)
    [1000230] = "defaultTalkWithMiounne_001",               -- Miounne              (Gridania: Adv. Guild)
    [1000231] = "defaultTalkWithHereward_001",              -- Hereward             (Gridania: LTW Guild)
    [1000234] = "defaultTalkWithSolieine_001",              -- Soileine 			(Gridania: CNJ Guild) [function typo] Has Parley actor id: 1700030
    [1000236] = "defaultTalkWithOpyltyl_001",               -- Opyltyl              (Gridania: BTN Guild)
    [1000238] = "defaultTalkWithPowle_001",                 -- Powle 				(Gridania: Acorn Orchard) - Has many actorclass IDs, this is the first one.
    [1000239] = "defaultTalkWithSansa_001",                 -- Sansa 				(Gridania: Acorn Orchard)
    [1000242] = "defaultTalkWithWillelda_001",              -- Willelda             (Gridania: LNC Guild) defaultTalkWithWillelda_002 - After signing up to the guild?
    [1000243] = "defaultTalkWithBurchard_001",              -- Burchard             (Gridania: LNC Guild)
    [1000326] = "defaultTalkWithCicely_001",                -- Cicely               (Gridania: BTN Guild)
    [1000409] = "defaultTalkWithNicoliaux_001",             -- Nicoliaux 			(Gridania: Acorn Orchard) [has multiple map markers, one might be regular idle location?]
    [1000410] = "defaultTalkWithAunillie_001",              -- Aunillie 			(Gridania: Acorn Orchard)
    [1000411] = "defaultTalkWithElyn_001",                  -- Elyn 				(Gridania: Acorn Orchard)
    [1000412] = "defaultTalkWithRyd_001",                   -- Ryd 					(Gridania: Acorn Orchard)
    [1000427] = "defaultTalkWithAnene_001",                 -- Anene                (Gridania: Adv. Guild) defaultTalkWithAnene_002 / 003 (PGL informant)
    [1000428] = "defaultTalkWithSylbyrt_001",               -- Sylbyrt              (Gridania: Adv. Guild) defaultTalkWithSylbyrt_002 / 003 (MRD informant)
    [1000429] = "defaultTalkWithHongavunga_001",            -- Honga Vunga          (Gridania: Adv. Guild) defaultTalkWithHongavunga_002 / 003 (WVR informant)
    [1000430] = "defaultTalkWithNoncomananco_001",          -- Nonco Menanco        (Gridania: Adv. Guild) arg1=1 or 21, mentions checking out DoW/M, otherwise DoH.  defaultTalkWithNoncomananco_002 / 003 (GSM informant)
    [1000431] = "defaultTalkWithLtandhaa_001",              -- L'tandhaa            (Gridania: Adv. Guild) defaultTalkWithLtandhaa_002 / 003 (ALC informant)
    [1000432] = "defaultTalkWithPofufu_001",                -- Pofufu               (Gridania: Adv. Guild) defaultTalkWithPofufu_002 / 003 (MIN informant)
    [1000433] = "defaultTalkWithDrividot_001",              -- Drividot             (Gridania: Adv. Guild) defaultTalkWithDrividot_002 / 003 (FSH informant)
    [1000434] = "defaultTalkWithOdilie_001",                -- Odilie               (Gridania: Adv. Guild) defaultTalkWithOdilie_002 / 003 (CUL informant)    
    [1000435] = "defaultTalkWithBasewin_001",               -- Basewin              (Gridania: Adv. Guild) defaultTalkWithBasewin_002 / 003 (BSM informant)
    [1000436] = "defaultTalkWithSeikfrae_001",              -- Seikfrae             (Gridania: Adv. Guild) defaultTalkWithSeikfrae_002 / 003 (GLD informant)
    [1000437] = "defaultTalkWithEdasshym_001",              -- E'dasshym            (Gridania: Adv. Guild) defaultTalkWithEdasshym_002 / 003 (THM informant)
    [1000458] = "defaultTalkWithVkorolon_001",              -- V'korolon            (Gridania: Adv. Guild) - Inn NPC. defaultTalkWithInn_Desk used when Inn unlocked
    --[1000460] = "defaultTalkWithHetzkin_001",               -- Hetzkin            (Gridania: CNJ Guild) Guildmark NPC - Will not fire, not PplStd.
    [1000463] = "defaultTalkWithNonolato_001",              -- Nonolato             (Gridania: ARC Guild) 
    [1000465] = "defaultTalkWithAnaidjaa_001",              -- A'naidjaa            (Gridania: CRP Guild)
    [1000504] = "defaultTalkWithTelent_001",                -- Telent 				(Gridania: CNJ Guild) - Has map marker, but whole-numbered.
    [1000509] = "defaultTalkWithKinborow_001",              -- Kinborow 			(Gridania: CNJ Guild) - Has marker
    [1000510] = "defaultTalkWithZerig_001",                 -- Zerig 				(Gridania: CNJ Guild) - Has map marker, but whole-numbered.
    [1000511] = "defaultTalkWithConcessa_001",              -- Concessa 			(Gridania: CNJ Guild)
    [1000512] = "defaultTalkWithMaroile_001",               -- Maroile 				(Gridania: CNJ Guild) - Has marker
    [1000513] = "defaultTalkWithGugula_001",                -- Gugula 				(Gridania: CNJ Guild)
    [1000556] = "defaultTalkWithWybir_001",                 -- <<<NOT IMPLEMENTED, HAS MARKER>>> Wybir (South Shroud: Quarrymill) - Has marker
    [1000565] = "defaultTalkWithCeinguled_001",             -- Ceinguled            (Gridania: LNC Guild)
    [1000566] = "defaultTalkWithFrancis_001",               -- Francis              (Gridania: LNC Guild) arg1=1, npc recognizes you're in the LNC guild
    [1000567] = "defaultTalkWithDhemdaeg_001",              -- Dhemdaeg             (Gridania: LNC Guild)
    [1000568] = "defaultTalkWithLuitfrid_001",              -- Luitfrid             (Gridania: LNC Guild)
    [1000569] = "defaultTalkWithHaurtefert_001",            -- Haurtefert           (Gridania: LNC Guild)
    [1000570] = "defaultTalkWithZpahtalo_001",              -- Z'pahtalo            (Gridania: LNC Guild)
    [1000599] = "defaultTalkWithJmoldva_001",               -- J'moldva             (Gridania: LNC Guild)
    [1000621] = "defaultTalkWithHabreham_001",              -- Habreham             (Gridania: CRP Guild)
    [1000622] = "defaultTalkWithDecima_001",                -- Decima               (Gridania: CRP Guild)
    [1000623] = "defaultTalkWithChalyotamlyo_001",          -- Chalyo Tamlyo        (Gridania: CRP Guild)
    [1000625] = "defaultTalkWithBubuku_001",                -- Bubuku               (Gridania: ARC Guild)
    [1000626] = "defaultTalkWithPiers_001",                 -- Piers                (Gridania: ARC Guild)
    [1000627] = "defaultTalkWithAerstsyn_001",              -- Aerstsyn             (Gridania: LNC Guild)
    [1000629] = "defaultTalkWithEburhart_001",              -- Eburhart             (Gridania: BTN Guild)
    [1000630] = "defaultTalkWithNoes_001",                  -- Noes                 (Gridania: Apkallus Falls)
    [1000669] = "defaultTalkWithJajajbygo_001",             -- <<<NOT IMPLEMENTED>>> Jajajbygo (Central Shroud: Camp Benchbranch) If Arg1 = 20 (SpecialEventWork correlation?), extra dialog about Atomos
    [1000670] = "defaultTalkWithPepeli_001",                -- <<<NOT IMPLEMENTED>>> Pepeli (Central Shroud: Camp Benchbranch) If Arg1 = 20 (SpecialEventWork correlation?), extra dialog about 7U Era starting
    [1000671] = "defaultTalkWithMiraudont_001",             -- Miraudont            (North Shroud: Camp Emerald Moss) arg1=true - Mentions Atomos
    [1000681] = "defaultTalkWithNuala_001",                 -- Nuala                (Gridania: LNC Guild)
    [1000701] = "defaultTalkWithZuzupoja_001",              -- Zuzupoja             (Gridania: CRP Guild)
    [1000737] = "defaultTalkWithBiddy_001",                 -- Biddy 				(Gridania: CNJ Guild) - Has map marker, but whole-numbered.
    [1000821] = "defaultTalkWithNellaure_001",              -- Nellaure             (Gridania: CRP Guild)
    [1000822] = "defaultTalkWithCaplan_001",                -- Caplan               (Gridania: CRP Guild)
    [1000823] = "defaultTalkWithUlmhylt_001",               -- Ulmhylt              (Gridania: CRP Guild)
    [1000829] = "defaultTalkWithOdhinek_001",               -- O'dhinek             (Gridania: ARC Guild)
    [1000830] = "defaultTalkWithGeorjeaux_001",             -- Georjeaux            (Gridania: ARC Guild) defaultTalkWithGeorjeaux_002 - Dialog when you're part of the guild?
    [1000831] = "defaultTalkWithAlaire_001",                -- Alaire               (Gridania: ARC Guild)
    [1000832] = "defaultTalkWithMianne_001",                -- Mianne               (Gridania: ARC Guild)
    [1000837] = "defaultTalkWithRdjongo_001",               -- R'djongo 			(Gridania: Stillglade Fane)
    [1000839] = "defaultTalkWithKhujazhwan_001",            -- Khuja Zhwan 			(Gridania: Stillglade Fane)
    [1000951] = "defaultTalkWithLonsygg_001",               -- Lonsygg              (Gridania: Blue Badger Gate)
    [1000978] = "defaultTalkWithGylbart_001",               -- <<<NOT IMPLEMENTED, HAS MARKER>>> Gylbart (South Shroud: Quarrymill)
    [1001071] = "defaultTalkWithTnbulea_001",               -- T'nbulea 			(Gridania: CNJ Guild)
    [1001072] = "defaultTalkWithFoforyo_001",               -- Foforyo 				(Gridania: CNJ Guild)
    [1001077] = "defaultTalkWithBeli_001",                  -- Beli                 (Gridania: LTW Guild)
    [1001078] = "defaultTalkWithMaddeline_001",             -- Maddeline            (Gridania: LTW Guild)
    [1001079] = "defaultTalkWithDyrstbrod_001",             -- Dyrstbrod            (Gridania: LTW Guild)
    [1001080] = "defaultTalkWithTatagoi_001",               -- Tatagoi              (Gridania: LTW Guild)
    [1001081] = "defaultTalkWithKhumamoshroca_001",         -- Khuma Moshroca       (Gridania: LTW Guild)
    [1001082] = "defaultTalkWithLuilda_001",                -- Luilda               (Gridania: LTW Guild)
    [1001101] = "defaultTalkWithVnabyano_001",              -- V'nabyano            (Gridania: BTN Guild)
    [1001102] = "defaultTalkWithSandre_001",                -- Sandre               (Gridania: BTN Guild)
    [1001103] = "defaultTalkWithMestonnaux_001",            -- Mestonnaux           (Gridania: BTN Guild)
    [1001150] = "defaultTalkWithBloisirant_001",            -- <<<NOT IMPLEMENTED, HAS MARKER>>> Bloisirant (South Shroud: Silent Arbor) Instance queue NPC for Toto-Rak - Will not fire, not PplStd. 
    [1001151] = "defaultTalkWithBidelia_001",               -- <<<NOT IMPLEMENTED>>> Bidelia - Entry Denier Guard?
    [1001152] = "defaultTalkWithDadaneja_001",              -- <<<NOT IMPLEMENTED>>> Dadaneja - Entry Denier Guard (West Shroud) - Guards fst_f0_dun06
    [1001153] = "defaultTalkWithRimomo_001",                -- <<<NOT IMPLEMENTED>>> Rimomo - Entry Denier Guard (North Shroud: 25,7)  - Guards fst_f0_dun05
    [1001175] = "defaultTalkWithChloe_001",                 -- Chloe                (Gridania: ARC Guild)
    [1001188] = "defaultTalkWithGuildleveClientG_001",      -- Maisenta             (Gridania)
    [1001189] = "defaultTalkWithGuildleveClientG_002",      -- Pukiki               (Gridania)
    [1001190] = "defaultTalkWithGuildleveClientG_003",      -- Eugenaire 			(Gridania: White Wolf Gate) - Has marker
    [1001294] = "defaultTalkWithIolaine_001",               -- <<<NOT IMPLEMENTED>>> Iolaine - Entry Denier Guard (West Shroud) - Also guards fst_f0_dun06
    [1001338] = "defaultTalkWithLivith_001",                -- <<<NOT IMPLEMENTED>>> Livith (North Shroud: Hyrstmill)
    [1001339] = "defaultTalkWithProscen_001",               -- <<<NOT IMPLEMENTED>>> Proscen (North Shroud: Hyrstmill)
    [1001340] = "defaultTalkWithTanguistl_001",             -- <<<NOT IMPLEMENTED>>> Tanguistl (North Shroud: Hyrstmill)
    [1001341] = "defaultTalkWithComoere_001",               -- <<<NOT IMPLEMENTED>>> Comoere (North Shroud: Hyrstmill) [dialog doesn't match wiki, but matching dialog isn't called in any function]
    [1001342] = "defaultTalkWithLougblaet_001",             -- <<<NOT IMPLEMENTED>>> Lougblaet (North Shroud: Hyrstmill)
    [1001343] = "defaultTalkWithFamushidumushi_001",        -- <<<NOT IMPLEMENTED>>> Famushi Dumushi (North Shroud: Hyrstmill)
    [1001344] = "defaultTalkWithDrystan_001",               -- <<<NOT IMPLEMENTED>>> Drystan (North Shroud: Hyrstmill)
    [1001345] = "defaultTalkWithEadbert_001",               -- <<<NOT IMPLEMENTED, HAS MARKER>>> Eadbert (North Shroud: Hyrstmill)
    [1001346] = "defaultTalkWithKeketo_001",                -- <<<NOT IMPLEMENTED, HAS MARKER>>> Keketo (South Shroud: Quarrymill)
    [1001347] = "defaultTalkWithRadianttear_001",           -- <<<NOT IMPLEMENTED>>> Radiant Tear (South Shroud: Quarrymill)
    [1001348] = "defaultTalkWithMyles_001",                 -- <<<NOT IMPLEMENTED>>> Myles (South Shroud: Quarrymill)
    [1001349] = "defaultTalkWithNathaniel_001",             -- <<<NOT IMPLEMENTED>>> Nathaniel (South Shroud: Quarrymill)
    [1001350] = "defaultTalkWithEvrardoux_001",             -- <<<NOT IMPLEMENTED>>> Evrardoux (South Shroud: Quarrymill)
    [1001351] = "defaultTalkWithTsehpanipahr_001",          -- <<<NOT IMPLEMENTED>>> Tseh Panipahr (South Shroud: Quarrymill)
    [1001352] = "defaultTalkWithEthelinda_001",             -- <<<NOT IMPLEMENTED, HAS MARKER>>> Ethelinda (South Shroud: Quarrymill)
    [1001353] = "defaultTalkWithHedheue_001",               -- <<<NOT IMPLEMENTED>>> Hedheue (South Shroud: Quarrymill)
    [1001396] = "defaultTalkWithLefwyne_001",               -- Lefwyne              (Gridania: Shaded Bower)
    [1001430] = "defaultTalkWithKinnison_001",              -- Kinnison             (Gridania: Stillglade Fane) Two args (nil errors client). If either >= 0, mentions you've met Kan-E-Senna (joined a GC).  Position inaccurate.
    [1001431] = "defaultTalkWithGenna_001",                 -- Genna                (Gridania: Mih Khetto's Amphitheatre)
    [1001432] = "defaultTalkWithMathye_001",                -- Mathye               (Gridania: Blue Badger Gate)
    [1001433] = "defaultTalkWithUlta_001",                  -- Ulta                 (Gridania: Blue Badger Gate)
    [1001434] = "defaultTalkWithNicia_001",                 -- Nicia 				(Gridania: White Wolf Gate)
    [1001435] = "defaultTalkWithBlandie_001",               -- Blandie 				(Gridania: White Wolf Gate)
    [1001436] = "defaultTalkWithOwyne_001",                 -- Owyne                (Gridania: Aetheryte Plaza)
    [1001437] = "defaultTalkWithSybell_001",                -- Sybell               (Gridania: Aetheryte Plaza)
    [1001459] = "defaultTalkWithFlavielle_001",             -- Flavielle            (Gridania: Adv. Guild) defaultTalkWithFlavielle_002 / 003 (ARM informant)
    [1001469] = "downTownTalk",                             -- Eldid                (Gridania: Wards Entrance)
    [1001470] = "defaultTalkWithYlessa_001",                -- Ylessa
    [1001570] = "defaultTalkWithRayao_001",                 -- <<<NOT IMPLEMENTED, HAS MARKER>>> Raya-O-Senna (North Shroud: Emerald Moss) WHM Job NPC, defaultTalkWithRayao_002
    [1001571] = "defaultTalkWithAruhnsenna_001",            -- <<<NOT IMPLEMENTED>>> A-Ruhn-Senna (Inside Toto-Rak instance)
    [1001582] = "defaultTalkWithSwaenhylt_001",             -- Swaenhylt            (Gridania)
    [1001583] = "defaultTalkWithMarcette_001",              -- Marcette             (Gridania: The Knot)
    [1001610] = "defaultTalkWithChamberliaux_001",          -- <<<NOT IMPLEMENTED, HAS MARKER>>> Chamberliaux (South Shroud: Buscarron's Fold)
    [1001611] = "defaultTalkWithFraemhar_001",              -- <<<NOT IMPLEMENTED>>> Fraemhar (East Shroud: Hawthorne Hut)
    [1001612] = "defaultTalkWithLora_001",                  -- <<<NOT IMPLEMENTED>>> Lora (East Shroud: Hawthorne Hut)
    [1001613] = "defaultTalkWithXbhowaqi_001",              -- <<<NOT IMPLEMENTED>>> X'bhowaqi (South Shroud: Buscarron's Fold)
    [1001614] = "defaultTalkWithWawaramu_001",              -- <<<NOT IMPLEMENTED>>> Wawaramu (South Shroud: Buscarron's Fold)
    [1001615] = "defaultTalkWithArnott_001",                -- <<<NOT IMPLEMENTED>>> Arnott (East Shroud: Hawthorne Hut)
    [1001620] = "talkIdayCap",                              -- <<<NOT IMPLEMENTED>>> Serpent Lieutenant Marette (Gridania: The Knot) - Foundation Day 2011 - OLD EVENT NPC: Replaced by 2012 version
    [1001621] = "talkIday1",                                -- <<<NOT IMPLEMENTED>>> Serpent Sergeant Frilaix (Gridania: The Knot) - Foundation Day 2011 - OLD EVENT NPC: Replaced by 2012 version
    [1001622] = "talkIday2",                                -- <<<NOT IMPLEMENTED>>> Serpent Private Tristelle (Gridania: The Knot) - Foundation Day 2011 - OLD EVENT NPC: Replaced by 2012 version
    [1001628] = "defaultTalkWithAilith_001",                -- <<<NOT IMPLEMENTED, HAS MARKER>>> Ailith (South Shroud: Quarrymill)
    [1001636] = "defaultTalkWithLhomujuuk_001",             -- <<<NOT IMPLEMENTED>>> Lho Mujuuk (South Shroud: Silent Arbor) - Hangs outside Toto-Rak entrance
    [1001637] = "defaultTalkWithSholnoralno_001",           -- <<<NOT IMPLEMENTED>>> Sholno Ralno (South Shroud: Silent Arbor) - Hangs outside Toto-Rak entrance
    [1001638] = "defaultTalkWithTuatkk_001",                -- <<<NOT IMPLEMENTED>>> Tuatkk (South Shroud: Silent Arbor) - Hangs outside Toto-Rak entrance
    [1001642] = "defaultTalkWithRonanKognan_001",           -- <<<NOT IMPLEMENTED>>> Ronan Kognan (Gridania: 5,5) - Has a variety of functions, listed under onTalk()
    [1001706] = "defaultTalkWithMemama_001",                -- Memama               (Gridania: Adv. Guild)
    [1001707] = "defaultTalkWithPfarahr_001",               -- Pfarahr              (Gridania: Adv. Guild)
    [1001708] = "defaultTalkWithBeaudonet_001",             -- Beaudonet            (Gridania: Adv. Guild)
    [1001709] = "defaultTalkWithFryswyde_001",              -- Fryswyde             (Gridania: Adv. Guild)
    [1001710] = "defaultTalkWithWillielmus_001",            -- Willielmus           (Gridania: Adv. Guild)
    [1001711] = "defaultTalkWithQZamqo_001",                -- Q'zamqo 				(Gridania: Airship Landing)
    [1001806] = "defaultTalkEnie_001",                      -- Enie                 (Gridania: BTN Guild)
    [1001835] = "defaultTalkWithVorsaile_001",              -- <<<NOT IMPLEMENTED, HAS MARKER>>> Serpent Commander Heuloix (North Shroud: Emerald Moss)
    [1001836] = "defaultTalkWithPukwapika_001",             -- <<<NOT IMPLEMENTED, HAS MARKER>>> Pukwa Pika (West Shroud: Turning Leaf) - Involved in "A Feast of Fools", Thornmarch fight
    [1001837] = "defaultTalkWithPurumoogle_001",            -- <<<NOT IMPLEMENTED>>> Frightened Moogle (West Shroud: Turning Leaf) - Hangs out beside Pukwa Pika
    [1001838] = "defaultTalkWithPirimoogle_001",            -- <<<NOT IMPLEMENTED>>> Fretful Moogle (West Shroud: Turning Leaf) - Hangs out beside Pukwa Pika
    [1001936] = "defaultTalkWithPukno_001",                 -- <<<NOT IMPLEMENTED, HAS MARKER>>> Pukno Poki - defaultTalkWithPukno_002 - Used after unlocking BRD?
    [1001937] = "defaultTalkWithMoogleA_001",               -- <<<NOT IMPLEMENTED>>> Pukni Pakk (North Shroud: Emerald Moss) - Hangs with WHM Job NPC - defaultTalkWithMoogleA_002 - Post-WHM dialog?
    [1001938] = "defaultTalkWithMppgleB_001",               -- <<<NOT IMPLEMENTED>>> Kupcha Kupa (North Shroud: Emerald Moss) - Hangs with WHM Job NPC - defaultTalkWithMppgleB_002 - Post-WHM dialog?
    [1001951] = "defaultTalkWithAnselm_001",                -- Anselm               (Gridania: Adv. Guild)
    [1001957] = "defaultTalkWithPukumoogle_001",            -- <<<NOT IMPLEMENTED>>> Plush Moogle (West Shroud: Crimson Bark)
    --[1002090] = "defaultTalkWithStewart_001",               -- Serpent Private Hodder (Gridania: Adv. Guild) defaultTalkWithStewart_002 (Post-Raid dialog?) - Will not fire, not PplStd.
    --[1002091] = "defaultTalkWithTrisselle_001",             -- Serpent Private Daurement (Gridania: Adv. Guild) defaultTalkWithTrisselle_002 (No idea for context) - Will not fire, not PplStd.
    [1002106] = "processEventELNAURE",                      -- Serpent Lieutenant Marette (Gridania: The Knot) - Foundation Day 2012 - Spl000 staticactor
    [1002107] = "processEventARISMONT",                     -- Serpent Sergeant Frilaix (Gridania: The Knot) - Foundation Day 2012 - Spl000 staticactor
    [1002108] = "processEventMERLIE",                       -- Serpent Private Tristelle (Gridania: The Knot) - Foundation Day 2012 - Spl000 staticactor
    [1060039] = "defaultTalkWithJehantel_001",              -- <<<NOT IMPLEMENTED, HAS MARKER>>> Jehantel (South Shroud: Tranquil Paths) BRD Job NPC - defaultTalkWithJehantel_002 
    [1060043] = "defaultTalkWithLegendBsm_001",             -- <<<NOT IMPLEMENTED, HAS MARKER>>> Gerolt (East Shroud: Hawthorne Hut) - Arg1 controls which line of dialog he plays, otherwise nothing shows
    --[1060022] = "defaultTalkLouisoix_001",                  -- Louisoix           (Gridania: Apkallus Falls) - Will not fire, not PplStd.
    [1200121] = "bookTalk",                                 -- Dusty Tomes  		(Gridania: CNJ Guild) - Will not fire since it isn't PplStd.  Identical dialog regardless.
    [1500055] = "defaultTalkWithLionnellais_001",           -- Lionnellais          (Gridania: Adv. Guild) - Will not fire, not PplStd.  Pre-airship dialog?
    [1500056] = "defaultTalkWithHida_001",                  -- Hida                 (Gridania: Adv. Guild) - Will not fire, not PplStd.  Pre-airship dialog?
    [1500060] = "defaultTalkWithHonoroit_001",              -- <<<NOT IMPLEMENTED>>> Honoroit (Central Shroud) - Hangs around (-200, 5, -810), has an untargetable chocobo carriage behind
    --[1500061] = "defaultTalkWithFhrudhem_001",            -- Fruhdhem             [function typo] (Gridania) Chocobo Taxi - Will not fire, not PplStd.
    [1500127] = "tribeTalk",                                -- Prosperlain          (Gridania)
    [1700001] = "defaultTalkWithPenelope_001",              -- Penelope             (Gridania: Adv. Guild)
    [1700038] = "defaultTalkWithAUBRENARD_100"              -- Aubrenard            (Gridania: Shaded Bower)
    
}
--[[ TO:DO - Map the remainder of these

defaultTalkWithAstrelle_001     -- "Astrelle" actor/name exists (1000736), but function calls blank dialog. Unused?  Perhaps Quest-only actor?
defQuest1g0_Bush                -- Empty function, unused? Perhaps Quest-only actor?
defQuest1g1_Bush                -- Empty function, unused? Perhaps Quest-only actor?


defaultTalkWithYonariumnari_001 -- "Yonari Umnari" actor/name exists (1000838), but cannot find existence of the npc or dialog on the internet.
defaultTalkWithMoogle010_001 -- No idea what moogles these are tied too.  
defaultTalkWithMoogle002_001 


defaultTalkCaravanChocoboGri_001
defaultTalkWithInn_ExitDoor
defaultTalkWithExit01
defaultTalkWithMarketNpc
defaultTalkWithHamletGuardGri_001
--]]


function onTalk(player, quest, npc, eventName)

    local npcId = npc:GetActorClassId();
    local clientFunc = defaultTalkFst[npcId];
    
    if (npcId == 1000430) then -- Nonco Menanco
        callClientFunction(player, "delegateEvent", player, quest, clientFunc, 21);
    elseif (npcId == 1000458) then -- V'korolon (Inn NPC)
        if (player:IsQuestCompleted(110828)) then -- "Waste Not Want Not" completed.
            defaultTalkWithInn(player, quest, "defaultTalkWithInn_Desk");
        else
            callClientFunction(player, "delegateEvent", player, quest, clientFunc);
        end        
    elseif (npcId == 1000669) then -- Jajajbygo
        callClientFunction(player, "delegateEvent", player, quest, clientFunc, 20);
    elseif (npcId == 1000670) then -- Pepeli
        callClientFunction(player, "delegateEvent", player, quest, clientFunc, 20);
    elseif (npcId == 1001430) then -- Kinnison
        callClientFunction(player, "delegateEvent", player, quest, clientFunc, -1,-1);
    elseif (npcId == 1001642) then -- Ronan Kognan
        callClientFunction(player, "delegateEvent", player, quest, clientFunc) -- Called if no deaspected crystals on player?
        --[[
        defaultTalkWithRonanKognan_002(bool1, bool2) -- Called if any deaspected crystals on player? bool1=Has enough deaspected for buying helmet bool2=already has helmet dialog
        defaultTalkWithRonanKognan_Hint_00  -- Lore dialog likely called in order as you make transactions with the npc?
        defaultTalkWithRonanKognan_Hint_01
        defaultTalkWithRonanKognan_Hint_02
        defaultTalkWithRonanKognan_Hint_03
        defaultTalkWithRonanKognan_Hint_04
        --]]
    elseif (npcId == 1001936) then -- Pukno Poki
        callClientFunction(player, "delegateEvent", player, quest, clientFunc); --defaultTalkWithPukno_002  -- Used after unlocking BRD?
    elseif (npcId == 1060039) then  -- Jehantel
        callClientFunction(player, "delegateEvent", player, quest, clientFunc); --defaultTalkWithJehantel_002  -- Post-BRD unlock?
    elseif (npcId == 1060043) then -- Gerolt
        callClientFunction(player, "delegateEvent", player, quest, clientFunc, 1);
    elseif ((npcId >= 1002106) and (npcId <= 1002108)) then  -- Foundation Day 2012 NPCs
        talkWithSpecial(player, npcId, clientFunc)
    else
        callClientFunction(player, "delegateEvent", player, quest, clientFunc); 
    end
    
    player:EndEvent();
end

function IsQuestENPC(player, quest, npc)
    return defaultTalkFst[npc:GetActorClassId()] ~= nil;
end

function defaultTalkWithInn(player, quest, clientFunc)
    local choice = callClientFunction(player, "delegateEvent", player, quest, clientFunc);
        
    if (choice == 1) then
        GetWorldManager():DoZoneChange(player, 244, nil, 0, 15, 160.048, 0, 154.263, 0);
    elseif (choice == 2) then           
        if (player:GetHomePointInn() ~= 2) then
            player:SetHomePointInn(2);
            player:SendGameMessage(GetWorldMaster(), 60019, 0x20, 2075); --Secondary homepoint set to the Roost
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
