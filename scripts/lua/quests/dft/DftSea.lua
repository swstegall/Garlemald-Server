require ("global")
--[[

Quest Script

Name: 	Small Talk
Code: 	DftSea
Id: 	110540

Contains all default lines for talkable npcs in the Sea Region (aka La Noscea).
* NOTE: This quest is active for all players at all times.
]]

-- [ActorClassId] = "client_function_name"
local defaultTalkSea = {

    [1000003] = "defaultTalkWithWaekbyrt_001",      -- Waekbyrt             (Limsa Lower Decks: MRD Guild) defaultTalkWithWaekbyrt_002 (post-MRD dialog?)
    [1000004] = "defaultTalkWithNunuba_001",        -- Nunuba               (Limsa Lower Decks: MRD Guild)
    [1000045] = "defaultTalkWithFabodji_001",       -- F'abodji             (Limsa Lower Decks)
    [1000049] = "defaultTalkWithJainelette_001",    -- Jainelette           (Limsa Lower Decks: MRD Guild)
    [1000050] = "defaultTalkWithRobairlain_001",    -- Robairlain           (Limsa Lower Decks)
    [1000051] = "defaultTalkWithBrictt_001",        -- Brictt               (Limsa Lower Decks: MRD Guild)
    [1000052] = "defaultTalkWithLiautroix_001",     -- Liautroix            (Limsa Lower Decks: MRD Guild)
    [1000053] = "defaultTalkWithSlaiboli_001",      -- S'laiboli            (Limsa Upper Decks: MSK Guild)
    [1000054] = "defaultTalkWithSyhrdaeg_001",      -- Syhrdaeg             (Limsa Upper Decks: MSK Guild)
    [1000056] = "defaultTalkWithLaniaitte_001",     -- Laniaitte            (Limsa Lower Decks: ACN Guild)
    [1000057] = "defaultTalkWithZonggo_001",        -- Z'onggo              (Limsa Lower Decks: FSH Guild)
    [1000060] = "defaultTalkWithPfynhaemr_001",     -- Pfynhaemr            (Limsa Upper Decks: BSM/ARM Guild)
    [1000061] = "defaultTalkWithMzimzizi_001",      -- M'zimzizi            (Limsa Upper Decks: BSM/ARM Guild)
    [1000062] = "defaultTalkWithCarrilaut_001",     -- Carrilaut            (Limsa Upper Decks: BSM/ARM Guild)
    [1000063] = "defaultTalkWithGerulf_001",        -- Gerulf               (Limsa Upper Decks: CUL Guild)
    [1000064] = "defaultTalkWithAentfoet_001",      -- Aentfoet             (Limsa Upper Decks: CUL Guild)
    [1000065] = "defaultTalkWithFrailoise_001",     -- Frailoise            (Limsa Upper Decks: CUL Guild)
    -- [1000078] = "defaultTalkWithAshakkal_001",   -- A'shakkal            (Limsa Upper Decks: Adv. Guild)  - Will not fire, not PplStd. Also blank strings.   
    [1000090] = "defaultTalkWithNeale_001",         -- Neale                (Limsa Lower Decks: MRD Guild)
    [1000125] = "defaultTalkWithChaunollet_001",    -- Chaunollet           (Limsa Upper Decks: MSK Guild)
    [1000129] = "defaultTalkWithRaragun_001",       -- Raragun              (Limsa Upper Decks: MSK Guild)
    [1000130] = "defaultTalkWithMynadaeg_001",      -- Mynadaeg             (Limsa Upper Decks: MSK Guild)
    [1000131] = "defaultTalkWithTefhmoshroca_001",  -- Tefh Moshroca        (Limsa Upper Decks: MSK Guild)
    [1000132] = "defaultTalkWithGinnade_001",       -- Ginnade              (Limsa Upper Decks: MSK Guild)
    [1000133] = "defaultTalkWithArthurioux_001",    -- Arthurioux           (Limsa Upper Decks: MSK Guild)
    [1000134] = "defaultTalkWithMartiallais_001",   -- Martiallais          (Limsa Upper Decks: BSM/ARM Guild)
    [1000135] = "defaultTalkWithIofa_001",          -- Iofa                 (Limsa Upper Decks: BSM/ARM Guild)
    [1000136] = "defaultTalkWithNanapiri_001",      -- Nanapiri             (Limsa Lower Decks: MRD Guild)
    [1000137] = "defaultTalkWithBaderon_001",       -- Baderon              (Limsa Upper Decks: Adv. Guild)
    [1000138] = "defaultTalkWithCharlys_001",       -- Charlys              (Limsa Upper Decks: CUL Guild)
    [1000144] = "defaultTalkWithBodenolf_001",      -- Bodenolf             (Limsa Upper Decks: BSM/ARM Guild) defaultTalkWithBodenolf_002 (post-BSM dialog?)
    [1000150] = "defaultTalkWithP_tahjha_001",      -- P'tahjha             (Limsa Lower Decks: ACN Guild)
    [1000151] = "defaultTalkWithRubh_hob_001",      -- Hob                  (Limsa Lower Decks: Ferry Docks)
    [1000152] = "defaultTalkWithIsaudorel_001",     -- Isaudorel            (Limsa Upper Decks: MSK Guild)
    [1000153] = "defaultTalkWithNnmulika_001",      -- N'nmulika            (Limsa Upper Decks: CUL Guild)
    [1000157] = "defaultTalkWithSraemha_001",       -- S'raemha             (Limsa Upper Decks: MRD Guild) - Will not fire, not PplStd.
    [1000158] = "defaultTalkWithNoline_001",        -- Noline               (Limsa Upper Decks: CUL Guild) - Will not fire, not PplStd.
    [1000159] = "defaultTalkWithJossy_001",         -- Jossy                (Limsa Upper Decks: CUL Guild) - Will not fire, not PplStd.
    [1000160] = "defaultTalkWithHaldberk_001",      -- Haldberk             (Limsa Lower Decks: ACN Guild)
    [1000161] = "defaultTalkWithTotoruto_001",      -- Totoruto             (Limsa Upper Decks: MSK Guild)
    [1000162] = "defaultTalkWithQhaschalahko_001",  -- Qhas Chalahko        (Limsa Upper Decks: BSM/ARM Guild) - Will not fire, not PplStd
    [1000163] = "defaultTalkWithJoellaut_001",      -- Joellaut             (Limsa Upper Decks: BSM/ARM Guild) - Will not fire, not PplStd
    [1000164] = "defaultTalkWithFaucillien_001",    -- Faucillien           (Limsa Upper Decks: FSH Guild) - Will not fire, not PplStd
    [1000165] = "defaultTalkWithLouviaune_001",     -- Louviaune            (Limsa Upper Decks: FSH Guild) - Will not fire, not PplStd
    [1000166] = "defaultTalkWithUrsulie_001",       -- Ursulie              (Limsa Upper Decks: Adv. Guild) - Will not fire, not PplStd.  Retainer NPC
    [1000167] = "defaultTalkWithMytesyn_001",       -- Mytesyn              (Limsa Upper Decks: Adv. Guild)  defaultTalkWithInn_Desk - used when Inn unlocked
    [1000168] = "defaultTalkWithPrudentia_001",     -- Prudentia            (Limsa Upper Decks: CUL Guild)
    [1000169] = "defaultTalkWithPulmia_001",        -- Pulmia               (Limsa Upper Decks: CUL Guild)
    [1000170] = "defaultTalkWithRsushmo_001",       -- R'sushmo             (Limsa Upper Decks: CUL Guild)
    [1000171] = "defaultTalkWithKikichua_001",      -- Kikichua             (Limsa Upper Decks: CUL Guild)
    [1000172] = "defaultTalkWithHobriaut_001",      -- Hobriaut             (Limsa Upper Decks: CUL Guild)
    [1000173] = "defaultTalkWithMaisie_001",        -- Maisie               (Limsa Lower Decks: FSH Guild)
    [1000177] = "defaultTalkWithSyngsmyd_001",      -- Syngsmyd             (Limsa Upper Decks: BSM/ARM Guild)
    [1000178] = "defaultTalkWithLilina_001",        -- Lilina               (Limsa Lower Decks: ACN Guild)
    [1000179] = "defaultTalkWithRubh_epocan_001",   -- Rubh Epocan          (Limsa Lower Decks: ACN Guild)
    [1000180] = "defaultTalkWithAstrid_001",        -- Astrid               (Limsa Lower Decks: FSH Guild)
    [1000181] = "defaultTalkWithXavalien_001",      -- Xavalien             (Limsa Lower Decks: FSH Guild)
    [1000190] = "defaultTalkWithBayard_001",        -- Bayard               (Limsa Lower Decks: MRD Guild)
    [1000191] = "defaultTalkWithTriaine_001",       -- Triaine              (Limsa Lower Decks: MRD Guild)
    [1000192] = "defaultTalkWithWyrakhamazom_001",  -- Wyra Khamazom        (Limsa Lower Decks: MRD Guild)
    [1000193] = "defaultTalkWithDhemsunn_001",      -- Dhemsunn             (Limsa Lower Decks: MRD Guild)
    [1000194] = "defaultTalkWithOsitha_001",        -- Ositha               (Limsa Lower Decks: MRD Guild)
    [1000195] = "defaultTalkWithElilwaen_001",      -- Elilwaen             (Limsa Lower Decks: ACN Guild)
    [1000196] = "defaultTalkWithDodoroba_001",      -- Dodoroba             (Limsa Lower Decks: ACN Guild)
    [1000197] = "defaultTalkWithIvan_001",          -- Ivan                 (Limsa Lower Decks: ACN Guild)
    [1000198] = "defaultTalkWithThosinbaen_001",    -- Thosinbaen           (Limsa Lower Decks: ACN Guild)
    [1000199] = "defaultTalkWithClifton_001",       -- Clifton              (Limsa Lower Decks: FSH Guild)
    [1000200] = "defaultTalkWithUndsatz_001",       -- Undsatz              (Limsa Lower Decks: FSH Guild)
    [1000201] = "defaultTalkWithRerenasu_001",      -- Rerenasu             (Limsa Lower Decks)
    [1000202] = "defaultTalkWithDacajinjahl_001",   -- Daca Jinjahl         (Limsa Lower Decks: FSH Guild)
    [1000203] = "defaultTalkWithBloemerl_001",      -- Bloemerl             (Limsa Lower Decks: FSH Guild)
    [1000217] = "defaultTalkWithChichiroon_001",    -- Chichiroon           (Limsa Lower Decks: MRD Guild)
    [1000219] = "defaultTalkWithBuburoon_001",      -- Buburoon             (Limsa Lower Decks: MRD Guild)
    [1000220] = "defaultTalkWithJojoroon_001",      -- Jojoroon             (Limsa Lower Decks: MRD Guild)
    [1000221] = "defaultTalkWithMimiroon_001",      -- Mimiroon             (Limsa Lower Decks: MRD Guild)
    [1000225] = "defaultTalkWithZehrymm_001",       -- Zehrymm              (Limsa Upper Decks)
    [1000226] = "defaultTalkWithFzhumii_001",       -- F'zhumii             (Limsa Lower Decks: Bulwark Hall)
    [1000227] = "defaultTalkWithArnegis_001",       -- Arnegis              (Limsa Lower Decks)
    [1000248] = "defaultTalkWithNheujawantal_001",  -- Nheu Jawantal        (Limsa Upper Decks: MSK Guild)
    [1000250] = "defaultTalkWithH_lahono_001",      -- H'lahono             (Limsa Upper Decks: CUL Guild)
    [1000252] = "defaultTalkWithWyrstmann_001",     -- Wyrstmann            (Limsa Upper Decks: CUL Guild)
    [1000253] = "defaultTalkWithTraveler030_001",   -- Tittering Traveler   (Limsa Upper Decks: CUL Guild)
    [1000254] = "defaultTalkWithTraveler031_001",   -- Suspicious-looking Traveler (Limsa Upper Decks: CUL Guild)
    [1000255] = "defaultTalkWithTraveler032_001",   -- Enraptured Traveler  (Limsa Upper Decks: CUL Guild)
    [1000256] = "defaultTalkWithYouty001_001",      -- Fickle Beggar        (Limsa Upper Decks: CUL Guild)
    [1000257] = "defaultTalkWithMerchant002_001",   -- Satiated Shopkeep    (Limsa Upper Decks: CUL Guild)
    [1000258] = "defaultTalkWithPirate030_001",     -- Pissed Pirate        (Limsa Upper Decks: CUL Guild)
    [1000259] = "defaultTalkWithLady002_001",       -- Overweening Woman    (Limsa Upper Decks: CUL Guild)
    [1000260] = "defaultTalkWithPorter001_001",     -- Pearly-toothed Porter(Limsa Lower Decks: Ferry Docks)
    [1000261] = "defaultTalkWithSailor031_001",     -- Muscle-bound Deckhand(Limsa Lower Decks: Ferry Docks)
    [1000262] = "defaultTalkWithLady001_001",       -- Glowing Goodwife     (Limsa Lower Decks: ACN Guild)
    [1000264] = "defaultTalkWithAdventurer030_001", -- Pasty-faced Adventurer (Limsa Lower Decks: Ferry Docks)
    [1000265] = "defaultTalkWithSosoze_001",        -- Sosoze               (Limsa Upper Decks: BSM/ARM Guild)
    [1000266] = "defaultTalkWithColson_001",        -- Colson               (Limsa Upper Decks: BSM/ARM Guild)
    [1000267] = "defaultTalkWithHihine_001",        -- Hihine               (Limsa Upper Decks: BSM/ARM Guild)
    [1000268] = "defaultTalkWithTrinne_001",        -- Trinne               (Limsa Upper Decks: BSM/ARM Guild)
    [1000269] = "defaultTalkWithKokoto_001",        -- Kokoto               (Limsa Upper Decks: Adv. Guild) defaultTalkWithKokoto_002 / 003 (GLD informant)
    [1000270] = "defaultTalkWithGigirya_001",       -- Gigirya              (Limsa Upper Decks: Adv. Guild) defaultTalkWithGigirya_002 / 003 (THM informant)
    [1000271] = "defaultTalkWithMaunie_001",        -- Maunie               (Limsa Upper Decks: Adv. Guild) defaultTalkWithMaunie_002 / 003 (PGL informant)
    [1000272] = "defaultTalkWithTirauland_001",     -- Tirauland            (Limsa Upper Decks: Adv. Guild) 001 (on non-LNC DoW/M) 010 (on DoH) 002 / 003 (LNC informant)
    [1000273] = "defaultTalkWithEstrilda_001",      -- Estrilda             (Limsa Upper Decks: Adv. Guild) defaultTalkWithEstrilda_002 / 003 (ARC informant)
    [1000274] = "defaultTalkWithGregory_001",       -- Gregory              (Limsa Upper Decks: Adv. Guild) defaultTalkWithGregory_002 / 003 (CNJ informant)
    [1000275] = "defaultTalkWithChantine_001",      -- Chantine             (Limsa Upper Decks: Adv. Guild) defaultTalkWithChantine_002 / 003 (WVR informant)
    [1000276] = "defaultTalkWithNanaka_001",        -- Nanaka               (Limsa Upper Decks: Adv. Guild) defaultTalkWithNanaka_002 / 003 (GSM informant)
    [1000277] = "defaultTalkWithKakamehi_001",      -- Kakamehi             (Limsa Upper Decks: Adv. Guild) defaultTalkWithKakamehi_002 / 003 (ALC informant)
    [1000278] = "defaultTalkWithStephannot_001",    -- Stephannot           (Limsa Upper Decks: Adv. Guild) defaultTalkWithStephannot_002 / 003 (MIN informant)
    [1000279] = "defaultTalkWithJosias_001",        -- Josias               (Limsa Upper Decks: Adv. Guild) defaultTalkWithJosias_002 / 003 (CRP informant)
    [1000280] = "defaultTalkWithFrithuric_001",     -- Frithuric            (Limsa Upper Decks: Adv. Guild) defaultTalkWithFrithuric_002 / 003 (LTW informant)
    [1000281] = "defaultTalkWithLauda_001",         -- Lauda                (Limsa Upper Decks: Adv. Guild) defaultTalkWithLauda_002 / 003 (BTN informant)
    [1000282] = "defaultTalkWithAdventurer031_001", -- Drowsy-eyed Adventurer (Limsa Lower Decks: Ferry Docks)
    [1000283] = "defaultTalkWithAdventurer032_001", -- Unconscious Adventurer (Limsa Lower Decks: MRD Guild)
    [1000284] = "defaultTalkWithPirate031_001",     -- Positively Pungeant Pirate (Limsa Lower Decks: ACN Guild)
    [1000286] = "defaultTalkWithKob031_001",        -- Sure-voiced Barracude Knight (Limsa Lower Decks: MRD Guild) 
    [1000330] = "defaultTalkWithCeadda_001",        -- Ceadda               (Limsa Lower Decks: Ferry Docks)
    [1000331] = "defaultTalkWithDympna_001",        -- Dympna               (Limsa Upper Decks: Thundersquall Thundersticks)
    [1000332] = "defaultTalkWithAhldskyff_001",     -- Ahldskyf             (Limsa Lower Decks: Ferry Docks)
    [1000333] = "defaultTalkWithSkarnwaen_001",     -- Skarnwaen            (Limsa Lower Decks)
    [1000334] = "defaultTalkWithShoshoma_001",      -- Shoshoma             (Limsa Lower Decks)
    [1000335] = "defaultTalkWithBmallpa_001",       -- B'mallpa             (Limsa Upper Decks)
    [1000337] = "defaultTalkWithMaetistym_001",     -- Maetistym            (Limsa Lower Decks)
    [1000338] = "defaultTalkWithSathzant_001",      -- Sathzant             (Limsa Upper Decks)
    [1000339] = "defaultTalkWithGnibnpha_001",      -- G'nibnpha            (Limsa Upper Decks)
    [1000340] = "defaultTalkWithRbaharra_001",      -- R'baharra            (Limsa Lower Decks: Ferry Docks)
    [1000341] = "defaultTalkWithTatasako_001",      -- Tatasako             (Limsa Lower Decks)
    [1000342] = "defaultTalkWithJghonako_001",      -- J'ghonako            (Limsa Upper Decks)
    [1000344] = "defaultTalkWithFerdillaix_001",    -- Ferdillaix           (Limsa Upper Decks)
    [1000345] = "defaultTalkWithFufuna_001",        -- Fufuna               (Limsa Upper Decks: The Hyaline)
    [1000346] = "defaultTalkWithAudaine_001",       -- Audaine              (Limsa Lower Decks: East Hawkers' Alley)
    [1000347] = "defaultTalkWithAergwynt_001",      -- Aergwynt             (Limsa Lower Decks)
    [1000348] = "defaultTalkWithOrtolf_001",        -- Ortolf               (Limsa Lower Decks)
    [1000349] = "defaultTalkWithSundhimal_001",     -- Sundhimal            (Limsa Upper Decks: Aetheryte Plaza)
    [1000350] = "defaultTalkWithEugennoix_001",     -- Eugennoix            (Limsa Upper Decks: Aetheryte Plaza)
    [1000351] = "defaultTalkWithZanthael_001",      -- Zanthael             (Limsa Lower Decks: Bulwark Hall)
    [1000359] = "defaultTalkWithRyssfloh_001",      -- Ryssflog             (Lower La Noscea: Camp Bearded Rock) If Arg1 = 20 (SpecialEventWork correlation?), extra dialog about dire beasts
    [1000360] = "defaultTalkWithKihtgamduhla_001",  -- Kiht Gamduhla        (Lower La Noscea: Camp Bearded Rock) If Arg1 = 20 (SpecialEventWork correlation?), extra dialog about Atomos
    [1000362] = "defaultTalkWithSolelle_001",       -- Solelle              (Western La Noscea: Camp Skull Valley)
    [1000363] = "defaultTalkWithNorman_001",        -- <<<NOT IMPLEMENTED>>>  - Norman - Entry Denier Guard (Upper La Noscea: U'Ghamaro Mines entrance) Est. 97.103, 64.368, -2702.809  - Guarded the zone when it wasn't playable in older version
    [1000364] = "defaultTalkWithBaenskylt_001",     -- <<<NOT IMPLEMENTED>>>  - Baenskylt - Entry Denier Guard (Eastern La Noscea) - Might have guarded dun05? (!warp 410 44 -847)
    [1000365] = "defaultTalkWithGautzelin_001",     -- Gautzelin            (Limsa Upper Decks: BSM/ARM Guild)
    [1000366] = "defaultTalkWithAimiliens_001",     -- <<<NOT IMPLEMENTED>>>  - Aimiliens - Entry Denier Guard (Western La Noscea) - Guards dun02 (Est. X:-1886.068 Y:22.445 Z:-849.989)
    [1000367] = "defaultTalkWithFongho_001",        -- <<<NOT IMPLEMENTED, HAS QUEST MARKER>>>  - F'ongho - (Lower La Noscea) - Hangs at Shposhae's entrance. Arg1=False adds dialog about you finding Shposhae.
    [1000468] = "defaultTalkWithVhynho_001",        -- V'hynho              (Limsa Upper Decks: MSK Guild)
    [1000469] = "defaultTalkWithZuzule_001",        -- Zuzule               (Limsa Upper Decks: MSK Guild)
    [1000470] = "defaultTalkWithFuzakanzak_001",    -- Fuzak Anzak          (Limsa Upper Decks: MSK Guild)
    [1000471] = "defaultTalkWithBnhapla_001",       -- B'nhapla             (Limsa Upper Decks: MSK Guild)
    [1000472] = "defaultTalkWithMerlzirn_001",      -- Merlzirn             (Limsa Upper Decks: MSK Guild) defaultTalkWithMerlzirn_002 (only plays the first msg of 001)
    [1000473] = "defaultTalkWithNinianne_001",      -- Ninianne             (Limsa Upper Decks: MSK Guild)
    [1000474] = "defaultTalkWithKehdamujuuk_001",   -- Kehda Mujuuk         (Limsa Lower Decks: ACN Guild)
    [1000475] = "defaultTalkWithWhahtoa_001",       -- W'hahtoa             (Limsa Lower Decks: MRD Guild)
    [1000476] = "defaultTalkWithGnanghal_001",      -- G'nanghal            (Limsa Lower Decks: MRD Guild)
    -- [1000613] = "defaultTalkWithNahctahr_001",      -- Nahctahr             (Lower La Noscea: Camp Bearded Rock) - Blank dialog. Will not fire, not PplStd.
    -- [1000614] = "defaultTalkWithKokomui_001",       -- Kokomui              (Eastern La Noscea: Camp Bloodshore) - Blank dialog. Will not fire, not PplStd.
    -- [1000615] = "defaultTalkWithEptolmi_001",       -- E'ptolmi             (Western La Noscea: Camp Skull Valley) - Blank dialog. Will not fire, not PplStd.
    -- [1000616] = "defaultTalkWithZabinie_001",       -- Zabinie              (Western La Noscea: Camp Bald Knoll) - Blank dialog. Will not fire, not PplStd.
    [1000620] = "defaultTalkWithDeladomadalado_001",-- Delado Madalado      (Limsa Lower Decks: ACN Guild)
    [1000662] = "defaultTalkWithSkoefmynd_001",     -- Skoefmynd            (Limsa Lower Decks: FSH Guild)
    [1001063] = "defaultTalkWithMharelak_001",      -- Mharelak             (Limsa Lower Decks: MRD Guild)
    [1001064] = "defaultTalkWithHasthwab_001",      -- Hasthwab             (Limsa Lower Decks: MRD Guild)
    [1001065] = "defaultTalkWithIghiimoui_001",     -- Ighii Moui           (Limsa Lower Decks: MRD Guild)
    [1001185] = "defaultTalkWithDavyd_001",         -- Leveridge            (Limsa Lower Decks)
    [1001186] = "defaultTalkWithNnagali_001",       -- H'rhanbolo           (Limsa Lower Decks)
    [1001187] = "defaultTalkWithKakalan_001",       -- Bango Zango          (Limsa Upper Decks)
    [1001298] = "defaultTalkWithBubusha_001",       -- Bubusha              (Western La Noscea: Aleport)
    [1001299] = "defaultTalkWithOadebh_001",        -- O'adebh              (Western La Noscea: Aleport)
    [1001300] = "defaultTalkWithMyndeidin_001",     -- Myndeidin            (Western La Noscea: Aleport)
    [1001301] = "defaultTalkWithFupepe_001",        -- Fupepe               (Western La Noscea: Aleport)
    [1001302] = "defaultTalkWithModestmouse_001",   -- Immodest Mouse       (Western La Noscea: Aleport)
    [1001303] = "defaultTalkWithDuchesnelt_001",    -- Duchesnelt           (Western La Noscea: Aleport)
    [1001304] = "defaultTalkWithSkribskoef_001",    -- Skribskoef           (Western La Noscea: Aleport)
    [1001305] = "defaultTalkWithYalabali_001",      -- Y'alabali            (Western La Noscea: Aleport)
    [1001306] = "defaultTalkWithSyzfrusk_001",      -- <<<NOT IMPLEMENTED, HAS QUEST MARKER>>> - Syzfrusk (Eastern La Noscea: Wineport)
    [1001307] = "defaultTalkWithInairoh_001",       -- <<<NOT IMPLEMENTED, HAS QUEST MARKER>>> - I'nairoh (Eastern La Noscea: Wineport)
    [1001308] = "defaultTalkWithMagaswyn_001",      -- <<<NOT IMPLEMENTED>>> - Magaswyn (Eastern La Noscea: Wineport)
    [1001309] = "defaultTalkWithSenahchalahko_001", -- <<<NOT IMPLEMENTED>>> - Senah Chalahko (Eastern La Noscea: Wineport)
    [1001310] = "defaultTalkWithWaldibert_001",     -- <<<NOT IMPLEMENTED>>> - Waldibert (Eastern La Noscea: Wineport)
    [1001311] = "defaultTalkWithEbandala_001",      -- <<<NOT IMPLEMENTED>>> - E'bandala (Eastern La Noscea: Wineport)
    [1001312] = "defaultTalkWithGuidingstar_001",   -- <<<NOT IMPLEMENTED>>> - Guiding Star (Eastern La Noscea: Wineport)
    [1001313] = "defaultTalkWithHundredeyes_001",   -- <<<NOT IMPLEMENTED>>> - Hundred Eyes (Eastern La Noscea: Wineport)
    [1001473] = "downTownTalk",                     -- Thata Khamazom       (Limsa Upper Decks) defaultTalkWithThatakhamazom_001 - Old function?
    [1001474] = "defaultTalkWithRoostingcrow_001",  -- Roosting Crow        (Limsa Upper Decks)
    [1001508] = "defaultTalkWithMareillie_001",     -- Mareillie            (Limsa Lower Decks)
    [1001509] = "defaultTalkWithSyntberk_001",      -- Syntberk             (Limsa Lower Decks)
    [1001510] = "defaultTalkWithAngryriver_001",    -- Angry River          (Limsa Lower Decks: ACN Guild)
    [1001511] = "defaultTalkWithBibiraka_001",      -- Bibiraka             (Limsa Lower Decks)
    [1001567] = "defaultTalkWithImania_001",        -- Imania               (Limsa Upper Decks)
    [1001573] = "defaultTalkWithSweetnix_001",      -- Sweetnix Rosycheeks  (Limsa Lower Decks)
    [1001603] = "defaultTalkWithLolojo_001",        -- <<<NOT IMPLEMENTED, HAS QUEST MARKER>>> Lolojo (Eastern La Noscea: Red Rooster Stead)
    [1001604] = "defaultTalkWithQmolosi_001",       -- <<<NOT IMPLEMENTED>>> Q'molosi (Western La Noscea: Halfstone)
    [1001605] = "defaultTalkWithBran_001",          -- <<<NOT IMPLEMENTED>>> Bran (Western La Noscea: Halfstone)
    [1001606] = "defaultTalkWithTutumoko_001",      -- <<<NOT IMPLEMENTED>>> Tutumoko (Eastern La Noscea: Red Rooster Stead)
    [1001607] = "defaultTalkWithBrianna_001",       -- <<<NOT IMPLEMENTED>>> Brianna (Eastern La Noscea: Red Rooster Stead)
    [1001608] = "defaultTalkWithFaine_001",         -- <<<NOT IMPLEMENTED, HAS QUEST MARKER>>> Faine (Western La Noscea: Halfstone)
    [1001609] = "defaultTalkWithAerghaemr_001",     -- <<<NOT IMPLEMENTED>>> Aerghaemr (Western La Noscea: Halfstone)
    [1001616] = "talkIdayCap",                      -- <<<NOT IMPLEMENTED>>> Storm Lieutenant Hardil (Limsa: Foundation Day 2011) - OLD EVENT NPC: Replaced by 2012 version
    [1001617] = "talkIday1",                        -- <<<NOT IMPLEMENTED>>> Storm Sergeant Allond (Limsa: Foundation Day 2011) - OLD EVENT NPC: Replaced by 2012 version
    [1001618] = "talkIday2",                        -- <<<NOT IMPLEMENTED>>> Storm Private Dracht (Limsa: Foundation Day 2011) - OLD EVENT NPC: Replaced by 2012 version
    [1001629] = "defaultTalkWithWalcher_001",       -- <<<NOT IMPLEMENTED, HAS QUEST MARKER>>> - Walcher (Eastern La Noscea: Wineport)
    [1001697] = "defaultTalkWithKurtz_001",         -- <<<HAS QUEST MARKER, VERIFY POS>>> - Storm Sergeant Nolan (Upper La Noscea: Camp Iron Lake)
    [1001700] = "defaultTalkWithAjinZukajin_001",   -- <<<NOT IMPLEMENTED>>> Ajin Zukajin (Limsa: Airship Landing)
    [1001701] = "defaultTalkWithRaplulu_001",       -- <<<NOT IMPLEMENTED>>> Raplulu (Limsa: Airship Landing)
    [1001702] = "defaultTalkWithZentsa_001",        -- <<<NOT IMPLEMENTED>>> G'zentsa (Limsa: Airship Landing)
    [1001703] = "defaultTalkWithAldyet_001",        -- <<<NOT IMPLEMENTED>>> Aldyet (Limsa: Airship Landing)
    [1001704] = "defaultTalkWithMurlskylt_001",     -- <<<NOT IMPLEMENTED>>> Murlskylt (Limsa: Airship Landing)
    [1001705] = "defaultTalkWith_Aenore001",        -- <<<NOT IMPLEMENTED>>> Aenore (Limsa: Airship Landing)
    [1001764] = "defaultTalkWithBaenryss_001",      -- <<<NOT IMPLEMENTED>>> Baenryss (Lower La Noscea) - Hangs at the Shposhae entrance
    [1001765] = "defaultTalkWithChachapi_001",      -- <<<NOT IMPLEMENTED>>> Chachapi (Lower La Noscea) - Hangs at the Shposhae entrance
    [1001766] = "defaultTalkWithForchetaix_001",    -- <<<NOT IMPLEMENTED>>> Forchetaix (Lower La Noscea) - Hangs at the Shposhae entrance
    [1001805] = "defaultTalkWithSizhaepocan_001",   -- <<<NOT IMPLEMENTED, HAS QUEST MARKER>>> - Sizha Epocan (Eastern La Noscea: Wineport)
    [1001833] = "defaultTalkWithAlain_001",         -- <<<NOT IMPLEMENTED, HAS QUEST MARKER>>> - Storm Sergeant Brooks (Upper La Noscea)
    [1500001] = "defaultTalkWithPiralnaut_001",     -- Piralnaut            (Limsa Upper Decks: Adv. Guild) - Will not fire, not PplStd.
    [1500003] = "defaultTalkWithFaezbroes_001",     -- <<<NOT IMPLEMENTED>>> - Faezbroes (Limsa: Airship Landing) - Will not fire, not PplStd.
    [1500004] = "defaultTalkWithGert_001",          -- Gert                 (Limsa Lower Decks: Ferry Docks)
    [1500005] = "defaultTalkWithLorhzant_001",      -- Lorhzant             (Limsa Lower Decks: Ferry Docks)
    [1500006] = "defaultTalkWithIsleen_001",        -- Isleen               (Limsa Lower Decks: Bulwark Hall) Pre-Chocobo rental dialog. Obsolete
    [1500125] = "tribeTalk",                        -- Merewina             (Limsa Lower Decks)
    [1700037] = "defaultTalkWithANSGOR_100",        -- Ansgor               (Limsa Lower Decks) <<Verify ID from caps, might be 1500117?>>
}


--[[ Need sourcing

    [1001619]  -- <<<NOT IMPLEMENTED>>> Storm Sergeant Solklinsyn (Limsa: Foundation Day 2011) - Cryer: Uses a different script

    defaultTalkStartMan -- Empty. startCliantTalkTurn & ends.  Likely debug.
    defaultTalkOiSAM    -- Empty. Likely debug.
    defaultTalkMLinhbo  -- Empty. Likely debug.

    defaultTalkWithNyaalamo_001  -- N'yaalamo?:  Airship NPC?  
        "All airships will remain in port until further notice. We apologize for the inconvenience."  
        Google giving me NOTHING. Mismatched NPC name?  Obsolete/Removed NPC?  Need to find a vid of the Limsa airship room.

    defaultTalkCaravanChocoboLim_001
    defaultTalkWithInn_ExitDoor(A0_767, A1_768, A2_769)
    defaultTalkWithExit01(A0_770, A1_771, A2_772)
    defaultTalkWithMarketNpc(A0_773, A1_774, A2_775)
    defaultTalkWithHamletGuardLim_001(A0_776, A1_777, A2_778)
--]]
    
    
 
function onTalk(player, quest, npc, eventName)

    local npcId = npc:GetActorClassId();
    local clientFunc = defaultTalkSea[npcId];
    
    if (npcId == 1000167) then -- Mytesyn - Inn NPC
        if (player:IsQuestCompleted(110838)) then -- "The Ink Thief" completed.
            defaultTalkWithInn(player, quest, "defaultTalkWithInn_Desk");
        else
            callClientFunction(player, "delegateEvent", player, quest, clientFunc);
        end
    else
        callClientFunction(player, "delegateEvent", player, quest, clientFunc);
    end
    
    player:EndEvent();
end

function IsQuestENPC(player, quest, npc)
	return defaultTalkSea[npc.GetActorClassId()] ~= nil;
end


function defaultTalkWithInn(player, quest, clientFunc)
    local choice = callClientFunction(player, "delegateEvent", player, quest, clientFunc);
        
    if (choice == 1) then
        GetWorldManager():DoZoneChange(player, 244, nil, 0, 15, -160.048, 0, -165.737, 0);
    elseif (choice == 2) then
        if (player:GetHomePointInn() ~= 1) then
            player:SetHomePointInn(3);
            player:SendGameMessage(GetWorldMaster(), 60019, 0x20, 1070); --Secondary homepoint set to the Mizzenmast
        else            
            player:SendGameMessage(GetWorldMaster(), 51140, 0x20); --This inn is already your Secondary Homepoint
        end
    end   
end