-- Garlemald SQLite schema. Applied once by `common::db::open_or_create` when
-- the database file does not yet exist. Columns mirror the Project Meteor
-- MySQL dumps (see ../../../project-meteor-mirror/Data/sql/*.sql) with
-- MySQL-specific types collapsed to SQLite's affinity-based types
-- (INTEGER / TEXT / REAL / BLOB). All column names preserve the original
-- camelCase so the existing Rust `SELECT`s match without aliasing.

CREATE TABLE IF NOT EXISTS characters (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    userId                  INTEGER NOT NULL,
    slot                    INTEGER NOT NULL,
    serverId                INTEGER NOT NULL,
    name                    TEXT NOT NULL,
    state                   INTEGER NOT NULL DEFAULT 0,
    creationDate            TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    isLegacy                INTEGER DEFAULT 0,
    doRename                INTEGER DEFAULT 0,
    playTime                INTEGER NOT NULL DEFAULT 0,
    positionX               REAL NOT NULL DEFAULT 0,
    positionY               REAL NOT NULL DEFAULT 0,
    positionZ               REAL NOT NULL DEFAULT 0,
    rotation                REAL NOT NULL DEFAULT 0,
    actorState              INTEGER DEFAULT 0,
    currentZoneId           INTEGER DEFAULT 0,
    currentPrivateArea      TEXT DEFAULT NULL,
    currentPrivateAreaType  INTEGER DEFAULT 0,
    destinationZoneId       INTEGER DEFAULT 0,
    destinationSpawnType    INTEGER DEFAULT 0,
    guardian                INTEGER DEFAULT 0,
    birthDay                INTEGER DEFAULT 0,
    birthMonth              INTEGER DEFAULT 0,
    initialTown             INTEGER DEFAULT 0,
    tribe                   INTEGER DEFAULT 0,
    gcCurrent               INTEGER DEFAULT 0,
    gcLimsaRank             INTEGER DEFAULT 127,
    gcGridaniaRank          INTEGER DEFAULT 127,
    gcUldahRank             INTEGER DEFAULT 127,
    currentTitle            INTEGER DEFAULT 0,
    restBonus               INTEGER DEFAULT 0,
    achievementPoints       INTEGER DEFAULT 0,
    currentActiveLinkshell  TEXT NOT NULL DEFAULT '',
    homepoint               INTEGER NOT NULL DEFAULT 0,
    homepointInn            INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS characters_appearance (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    characterId          INTEGER NOT NULL,
    baseId               INTEGER NOT NULL,
    size                 INTEGER NOT NULL DEFAULT 0,
    voice                INTEGER NOT NULL DEFAULT 0,
    skinColor            INTEGER NOT NULL DEFAULT 0,
    hairStyle            INTEGER NOT NULL DEFAULT 0,
    hairColor            INTEGER NOT NULL DEFAULT 0,
    hairHighlightColor   INTEGER NOT NULL DEFAULT 0,
    hairVariation        INTEGER NOT NULL DEFAULT 0,
    eyeColor             INTEGER NOT NULL DEFAULT 0,
    faceType             INTEGER NOT NULL DEFAULT 0,
    faceEyebrows         INTEGER NOT NULL DEFAULT 0,
    faceEyeShape         INTEGER NOT NULL DEFAULT 0,
    faceIrisSize         INTEGER NOT NULL DEFAULT 0,
    faceNose             INTEGER NOT NULL DEFAULT 0,
    faceMouth            INTEGER NOT NULL DEFAULT 0,
    faceFeatures         INTEGER NOT NULL DEFAULT 0,
    ears                 INTEGER NOT NULL DEFAULT 0,
    characteristics      INTEGER NOT NULL DEFAULT 0,
    characteristicsColor INTEGER NOT NULL DEFAULT 0,
    mainhand             INTEGER NOT NULL DEFAULT 0,
    offhand              INTEGER NOT NULL DEFAULT 0,
    head                 INTEGER NOT NULL DEFAULT 0,
    body                 INTEGER NOT NULL DEFAULT 0,
    hands                INTEGER NOT NULL DEFAULT 0,
    legs                 INTEGER NOT NULL DEFAULT 0,
    feet                 INTEGER NOT NULL DEFAULT 0,
    waist                INTEGER NOT NULL DEFAULT 0,
    neck                 INTEGER NOT NULL DEFAULT 0,
    leftIndex            INTEGER NOT NULL DEFAULT 0,
    rightIndex           INTEGER NOT NULL DEFAULT 0,
    leftFinger           INTEGER NOT NULL DEFAULT 0,
    rightFinger          INTEGER NOT NULL DEFAULT 0,
    leftEar              INTEGER NOT NULL DEFAULT 0,
    rightEar             INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS characters_class_levels (
    characterId INTEGER PRIMARY KEY,
    pug INTEGER DEFAULT 0, gla INTEGER DEFAULT 0, mrd INTEGER DEFAULT 0,
    arc INTEGER DEFAULT 0, lnc INTEGER DEFAULT 0, thm INTEGER DEFAULT 0,
    cnj INTEGER DEFAULT 0, crp INTEGER DEFAULT 0, bsm INTEGER DEFAULT 0,
    arm INTEGER DEFAULT 0, gsm INTEGER DEFAULT 0, ltw INTEGER DEFAULT 0,
    wvr INTEGER DEFAULT 0, alc INTEGER DEFAULT 0, cul INTEGER DEFAULT 0,
    min INTEGER DEFAULT 0, btn INTEGER DEFAULT 0, fsh INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS characters_class_exp (
    characterId INTEGER PRIMARY KEY,
    pug INTEGER DEFAULT 0, gla INTEGER DEFAULT 0, mrd INTEGER DEFAULT 0,
    arc INTEGER DEFAULT 0, lnc INTEGER DEFAULT 0, thm INTEGER DEFAULT 0,
    cnj INTEGER DEFAULT 0, crp INTEGER DEFAULT 0, bsm INTEGER DEFAULT 0,
    arm INTEGER DEFAULT 0, gsm INTEGER DEFAULT 0, ltw INTEGER DEFAULT 0,
    wvr INTEGER DEFAULT 0, alc INTEGER DEFAULT 0, cul INTEGER DEFAULT 0,
    min INTEGER DEFAULT 0, btn INTEGER DEFAULT 0, fsh INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS characters_hotbar (
    characterId INTEGER NOT NULL,
    classId     INTEGER NOT NULL,
    hotbarSlot  INTEGER NOT NULL,
    commandId   INTEGER NOT NULL,
    recastTime  INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (characterId, classId, hotbarSlot)
);

CREATE TABLE IF NOT EXISTS characters_parametersave (
    characterId     INTEGER PRIMARY KEY,
    hp              INTEGER NOT NULL DEFAULT 0,
    hpMax           INTEGER NOT NULL DEFAULT 0,
    mp              INTEGER NOT NULL DEFAULT 0,
    mpMax           INTEGER NOT NULL DEFAULT 0,
    mainSkill       INTEGER NOT NULL DEFAULT 0,
    mainSkillLevel  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS characters_chocobo (
    characterId       INTEGER PRIMARY KEY,
    hasChocobo        INTEGER DEFAULT 0,
    hasGoobbue        INTEGER DEFAULT 0,
    chocoboAppearance INTEGER DEFAULT NULL,
    chocoboName       TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS characters_inventory (
    characterId  INTEGER NOT NULL,
    serverItemId INTEGER NOT NULL,
    itemPackage  INTEGER NOT NULL,
    slot         INTEGER NOT NULL,
    PRIMARY KEY (characterId, serverItemId)
);

CREATE TABLE IF NOT EXISTS characters_inventory_equipment (
    characterId INTEGER NOT NULL,
    classId     INTEGER NOT NULL,
    equipSlot   INTEGER NOT NULL,
    itemId      INTEGER NOT NULL,
    PRIMARY KEY (characterId, classId, equipSlot)
);

CREATE TABLE IF NOT EXISTS characters_npclinkshell (
    characterId    INTEGER NOT NULL,
    npcLinkshellId INTEGER NOT NULL,
    isCalling      INTEGER NOT NULL DEFAULT 0,
    isExtra        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (characterId, npcLinkshellId)
);

CREATE TABLE IF NOT EXISTS characters_quest_completed (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    characterId INTEGER NOT NULL,
    questId     INTEGER NOT NULL,
    UNIQUE (characterId, questId)
);

CREATE TABLE IF NOT EXISTS characters_quest_guildleve_local (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    characterId INTEGER NOT NULL,
    slot        INTEGER NOT NULL,
    questId     INTEGER NOT NULL,
    abandoned   INTEGER DEFAULT 0,
    completed   INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS characters_quest_guildleve_regional (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    characterId INTEGER NOT NULL,
    slot        INTEGER NOT NULL,
    guildleveId INTEGER NOT NULL,
    abandoned   INTEGER DEFAULT 0,
    completed   INTEGER DEFAULT 0,
    UNIQUE (characterId, guildleveId)
);

CREATE TABLE IF NOT EXISTS characters_quest_scenario (
    characterId  INTEGER NOT NULL,
    slot         INTEGER NOT NULL,
    questId      INTEGER NOT NULL,
    currentPhase INTEGER NOT NULL DEFAULT 0,
    questData    TEXT,
    questFlags   INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (characterId, slot)
);

CREATE TABLE IF NOT EXISTS characters_statuseffect (
    characterId INTEGER NOT NULL,
    statusId    INTEGER NOT NULL,
    magnitude   INTEGER NOT NULL,
    duration    INTEGER NOT NULL,
    tick        INTEGER NOT NULL,
    tier        INTEGER NOT NULL,
    extra       INTEGER NOT NULL,
    PRIMARY KEY (characterId, statusId)
);

CREATE TABLE IF NOT EXISTS characters_timers (
    characterId            INTEGER PRIMARY KEY,
    thousandmaws           INTEGER DEFAULT 0,
    dzemaeldarkhold        INTEGER DEFAULT 0,
    bowlofembers_hard      INTEGER DEFAULT 0,
    bowlofembers           INTEGER DEFAULT 0,
    thornmarch             INTEGER DEFAULT 0,
    aurumvale              INTEGER DEFAULT 0,
    cutterscry             INTEGER DEFAULT 0,
    battle_aleport         INTEGER DEFAULT 0,
    battle_hyrstmill       INTEGER DEFAULT 0,
    battle_goldenbazaar    INTEGER DEFAULT 0,
    howlingeye_hard        INTEGER DEFAULT 0,
    howlingeye             INTEGER DEFAULT 0,
    castrumnovum           INTEGER DEFAULT 0,
    bowlofembers_extreme   INTEGER DEFAULT 0,
    rivenroad              INTEGER DEFAULT 0,
    rivenroad_hard         INTEGER DEFAULT 0,
    behests                INTEGER DEFAULT 0,
    companybehests         INTEGER DEFAULT 0,
    returntimer            INTEGER DEFAULT 0,
    skirmish               INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS characters_achievements (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    characterId    INTEGER NOT NULL,
    achievementId  INTEGER NOT NULL,
    timeDone       INTEGER DEFAULT NULL,
    progress       INTEGER DEFAULT 0,
    progressFlags  INTEGER DEFAULT 0,
    UNIQUE (characterId, achievementId)
);

CREATE TABLE IF NOT EXISTS characters_retainers (
    characterId INTEGER NOT NULL,
    retainerId  INTEGER NOT NULL,
    doRename    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (characterId, retainerId)
);

CREATE TABLE IF NOT EXISTS characters_linkshells (
    characterId INTEGER NOT NULL,
    linkshellId INTEGER NOT NULL,
    rank        INTEGER NOT NULL DEFAULT 4,
    PRIMARY KEY (characterId, linkshellId)
);

CREATE TABLE IF NOT EXISTS server_battle_commands (
    id                 INTEGER PRIMARY KEY,
    name               TEXT NOT NULL,
    classJob           INTEGER NOT NULL DEFAULT 0,
    lvl                INTEGER NOT NULL DEFAULT 0,
    requirements       INTEGER NOT NULL DEFAULT 0,
    mainTarget         INTEGER NOT NULL DEFAULT 0,
    validTarget        INTEGER NOT NULL DEFAULT 0,
    aoeType            INTEGER NOT NULL DEFAULT 0,
    aoeRange           REAL NOT NULL DEFAULT 0,
    aoeMinRange        REAL NOT NULL DEFAULT 0,
    aoeConeAngle       REAL NOT NULL DEFAULT 0,
    aoeRotateAngle     REAL NOT NULL DEFAULT 0,
    aoeTarget          INTEGER NOT NULL DEFAULT 0,
    basePotency        INTEGER NOT NULL DEFAULT 0,
    numHits            INTEGER NOT NULL DEFAULT 0,
    positionBonus      INTEGER NOT NULL DEFAULT 0,
    procRequirement    INTEGER NOT NULL DEFAULT 0,
    "range"            INTEGER NOT NULL DEFAULT 0,
    minRange           INTEGER NOT NULL DEFAULT 0,
    bestRange          INTEGER NOT NULL DEFAULT 0,
    rangeHeight        INTEGER NOT NULL DEFAULT 10,
    rangeWidth         INTEGER NOT NULL DEFAULT 2,
    statusId           INTEGER NOT NULL DEFAULT 0,
    statusDuration     INTEGER NOT NULL DEFAULT 0,
    statusChance       REAL NOT NULL DEFAULT 0.5,
    castType           INTEGER NOT NULL DEFAULT 0,
    castTime           INTEGER NOT NULL DEFAULT 0,
    recastTime         INTEGER NOT NULL DEFAULT 0,
    mpCost             INTEGER NOT NULL DEFAULT 0,
    tpCost             INTEGER NOT NULL DEFAULT 0,
    animationType      INTEGER NOT NULL DEFAULT 0,
    effectAnimation    INTEGER NOT NULL DEFAULT 0,
    modelAnimation     INTEGER NOT NULL DEFAULT 0,
    animationDuration  INTEGER NOT NULL DEFAULT 0,
    battleAnimation    INTEGER NOT NULL DEFAULT 0,
    validUser          INTEGER NOT NULL DEFAULT 0,
    comboId1           INTEGER NOT NULL DEFAULT 0,
    comboId2           INTEGER NOT NULL DEFAULT 0,
    comboStep          INTEGER NOT NULL DEFAULT 0,
    accuracyMod        REAL NOT NULL DEFAULT 1,
    worldMasterTextId  INTEGER NOT NULL DEFAULT 0,
    commandType        INTEGER NOT NULL DEFAULT 0,
    actionType         INTEGER NOT NULL DEFAULT 0,
    actionProperty     INTEGER NOT NULL DEFAULT 0,
    isRanged           INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS server_battle_traits (
    id       INTEGER PRIMARY KEY,
    name     TEXT NOT NULL,
    classJob INTEGER NOT NULL,
    lvl      INTEGER NOT NULL,
    modifier INTEGER NOT NULL DEFAULT 0,
    bonus    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS server_statuseffects (
    id               INTEGER PRIMARY KEY,
    name             TEXT NOT NULL,
    flags            INTEGER NOT NULL DEFAULT 10,
    overwrite        INTEGER NOT NULL DEFAULT 1,
    tickMs           INTEGER NOT NULL DEFAULT 3000,
    hidden           INTEGER NOT NULL DEFAULT 0,
    silentOnGain     INTEGER NOT NULL DEFAULT 0,
    silentOnLoss     INTEGER NOT NULL DEFAULT 0,
    statusGainTextId INTEGER NOT NULL DEFAULT 30328,
    statusLossTextId INTEGER NOT NULL DEFAULT 30331
);

CREATE TABLE IF NOT EXISTS server_items (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    itemId   INTEGER NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 0,
    quality  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS server_items_dealing (
    id               INTEGER PRIMARY KEY,
    dealingValue     INTEGER NOT NULL DEFAULT 0,
    dealingMode      INTEGER NOT NULL DEFAULT 0,
    dealingAttached1 INTEGER DEFAULT 0,
    dealingAttached2 INTEGER NOT NULL DEFAULT 0,
    dealingAttached3 INTEGER NOT NULL DEFAULT 0,
    dealingTag       INTEGER NOT NULL DEFAULT 0,
    bazaarMode       INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS server_items_modifiers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    durability  INTEGER NOT NULL DEFAULT 0,
    mainQuality INTEGER NOT NULL DEFAULT 0,
    subQuality1 INTEGER NOT NULL DEFAULT 0,
    subQuality2 INTEGER NOT NULL DEFAULT 0,
    subQuality3 INTEGER NOT NULL DEFAULT 0,
    param1      INTEGER NOT NULL DEFAULT 0,
    param2      INTEGER NOT NULL DEFAULT 0,
    param3      INTEGER NOT NULL DEFAULT 0,
    spiritbind  INTEGER NOT NULL DEFAULT 0,
    materia1    INTEGER NOT NULL DEFAULT 0,
    materia2    INTEGER NOT NULL DEFAULT 0,
    materia3    INTEGER NOT NULL DEFAULT 0,
    materia4    INTEGER NOT NULL DEFAULT 0,
    materia5    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS server_linkshells (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    name      TEXT NOT NULL UNIQUE,
    crestIcon INTEGER NOT NULL DEFAULT 0,
    crest     INTEGER NOT NULL DEFAULT 0,
    master    INTEGER NOT NULL DEFAULT 0,
    rank      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS server_retainers (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         TEXT NOT NULL,
    actorClassId INTEGER NOT NULL,
    cdIDOffset   INTEGER NOT NULL DEFAULT 0,
    placeName    INTEGER NOT NULL DEFAULT 0,
    conditions   INTEGER NOT NULL DEFAULT 0,
    level        INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS server_seamless_zonechange_bounds (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    regionId             INTEGER NOT NULL,
    zoneId1              INTEGER NOT NULL,
    zoneId2              INTEGER NOT NULL,
    zone1_boundingbox_x1 REAL NOT NULL,
    zone1_boundingbox_y1 REAL NOT NULL,
    zone1_boundingbox_x2 REAL NOT NULL,
    zone1_boundingbox_y2 REAL NOT NULL,
    zone2_boundingbox_x1 REAL NOT NULL,
    zone2_boundingbox_x2 REAL NOT NULL,
    zone2_boundingbox_y1 REAL NOT NULL,
    zone2_boundingbox_y2 REAL NOT NULL,
    merge_boundingbox_x1 REAL NOT NULL,
    merge_boundingbox_y1 REAL NOT NULL,
    merge_boundingbox_x2 REAL NOT NULL,
    merge_boundingbox_y2 REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS server_zones (
    id             INTEGER PRIMARY KEY,
    regionId       INTEGER NOT NULL DEFAULT 0,
    zoneName       TEXT DEFAULT NULL,
    placeName      TEXT NOT NULL DEFAULT '',
    serverIp       TEXT NOT NULL DEFAULT '',
    serverPort     INTEGER NOT NULL DEFAULT 0,
    classPath      TEXT NOT NULL DEFAULT '',
    dayMusic       INTEGER DEFAULT 0,
    nightMusic     INTEGER DEFAULT 0,
    battleMusic    INTEGER DEFAULT 0,
    isIsolated     INTEGER DEFAULT 0,
    isInn          INTEGER DEFAULT 0,
    canRideChocobo INTEGER DEFAULT 1,
    canStealth     INTEGER DEFAULT 0,
    isInstanceRaid INTEGER DEFAULT 0,
    loadNavMesh    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS server_zones_privateareas (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    parentZoneId     INTEGER NOT NULL,
    className        TEXT NOT NULL DEFAULT '',
    privateAreaName  TEXT NOT NULL DEFAULT '',
    privateAreaType  INTEGER NOT NULL DEFAULT 0,
    dayMusic         INTEGER DEFAULT 0,
    nightMusic       INTEGER DEFAULT 0,
    battleMusic      INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS server_zones_spawnlocations (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    zoneId           INTEGER NOT NULL,
    privateAreaName  TEXT DEFAULT NULL,
    spawnType        INTEGER DEFAULT 0,
    spawnX           REAL NOT NULL DEFAULT 0,
    spawnY           REAL NOT NULL DEFAULT 0,
    spawnZ           REAL NOT NULL DEFAULT 0,
    spawnRotation    REAL NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS server_spawn_locations (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    actorClassId      INTEGER NOT NULL,
    uniqueId          TEXT NOT NULL DEFAULT '',
    zoneId            INTEGER NOT NULL,
    privateAreaName   TEXT NOT NULL DEFAULT '',
    privateAreaLevel  INTEGER NOT NULL DEFAULT 0,
    positionX         REAL NOT NULL DEFAULT 0,
    positionY         REAL NOT NULL DEFAULT 0,
    positionZ         REAL NOT NULL DEFAULT 0,
    rotation          REAL NOT NULL DEFAULT 0,
    actorState        INTEGER NOT NULL DEFAULT 0,
    animationId       INTEGER NOT NULL DEFAULT 0,
    customDisplayName TEXT DEFAULT NULL
);

CREATE TABLE IF NOT EXISTS supportdesk_tickets (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    name     TEXT NOT NULL,
    title    TEXT NOT NULL,
    body     TEXT NOT NULL,
    langCode INTEGER NOT NULL,
    isOpen   INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS supportdesk_faqs (
    slot         INTEGER NOT NULL,
    languageCode INTEGER NOT NULL,
    title        TEXT NOT NULL,
    body         TEXT NOT NULL,
    PRIMARY KEY (slot, languageCode)
);

CREATE TABLE IF NOT EXISTS supportdesk_issues (
    slot  INTEGER PRIMARY KEY,
    title TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS gamedata_actor_class (
    id              INTEGER PRIMARY KEY,
    classPath       TEXT NOT NULL,
    displayNameId   INTEGER NOT NULL DEFAULT 4294967295,
    propertyFlags   INTEGER NOT NULL DEFAULT 0,
    eventConditions TEXT
);

CREATE TABLE IF NOT EXISTS gamedata_actor_pushcommand (
    id                  INTEGER PRIMARY KEY,
    pushCommand         INTEGER NOT NULL DEFAULT 0,
    pushCommandSub      INTEGER NOT NULL DEFAULT 0,
    pushCommandPriority INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS gamedata_items (
    catalogID        INTEGER PRIMARY KEY,
    name             TEXT NOT NULL DEFAULT '',
    singular         TEXT DEFAULT '',
    plural           TEXT DEFAULT '',
    category         TEXT NOT NULL DEFAULT '',
    maxStack         INTEGER NOT NULL DEFAULT 0,
    stackSize        INTEGER NOT NULL DEFAULT 0,
    isRare           INTEGER NOT NULL DEFAULT 0,
    isExclusive      INTEGER NOT NULL DEFAULT 0,
    durability       INTEGER NOT NULL DEFAULT 0,
    sellPrice        INTEGER NOT NULL DEFAULT 0,
    buyPrice         INTEGER NOT NULL DEFAULT 0,
    price            INTEGER NOT NULL DEFAULT 0,
    icon             INTEGER NOT NULL DEFAULT 0,
    kind             INTEGER NOT NULL DEFAULT 0,
    rarity           INTEGER NOT NULL DEFAULT 0,
    isUseable        INTEGER NOT NULL DEFAULT 0,
    mainSkill        INTEGER NOT NULL DEFAULT 0,
    subSkill         INTEGER NOT NULL DEFAULT 0,
    levelType        INTEGER NOT NULL DEFAULT 0,
    level            INTEGER NOT NULL DEFAULT 0,
    itemLevel        INTEGER NOT NULL DEFAULT 0,
    equipLevel       INTEGER NOT NULL DEFAULT 0,
    itemUICategory   INTEGER NOT NULL DEFAULT 0,
    compatibility    INTEGER NOT NULL DEFAULT 0,
    effectMagnitude  REAL NOT NULL DEFAULT 0,
    effectRate       REAL NOT NULL DEFAULT 0,
    shieldBlocking   REAL NOT NULL DEFAULT 0,
    effectDuration   REAL NOT NULL DEFAULT 0,
    recastTime       REAL NOT NULL DEFAULT 0,
    recastGroup      INTEGER NOT NULL DEFAULT 0,
    repairSkill      INTEGER NOT NULL DEFAULT 0,
    repairItem       INTEGER NOT NULL DEFAULT 0,
    repairItemNum    INTEGER NOT NULL DEFAULT 0,
    repairLevel      INTEGER NOT NULL DEFAULT 0,
    repairLicense    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS gamedata_items_accessory (
    catalogID INTEGER PRIMARY KEY,
    power     INTEGER NOT NULL DEFAULT 0,
    size      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS gamedata_items_armor (
    catalogID            INTEGER PRIMARY KEY,
    defense              INTEGER NOT NULL DEFAULT 0,
    magicDefense         INTEGER NOT NULL DEFAULT 0,
    criticalDefense      INTEGER NOT NULL DEFAULT 0,
    evasion              INTEGER NOT NULL DEFAULT 0,
    magicResistance      INTEGER NOT NULL DEFAULT 0,
    damageDefenseType1   INTEGER NOT NULL DEFAULT 0,
    damageDefenseValue1  INTEGER NOT NULL DEFAULT 0,
    damageDefenseType2   INTEGER NOT NULL DEFAULT 0,
    damageDefenseValue2  INTEGER NOT NULL DEFAULT 0,
    damageDefenseType3   INTEGER NOT NULL DEFAULT 0,
    damageDefenseValue3  INTEGER NOT NULL DEFAULT 0,
    damageDefenseType4   INTEGER NOT NULL DEFAULT 0,
    damageDefenseValue4  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS gamedata_items_equipment (
    catalogID              INTEGER PRIMARY KEY,
    equipPoint             INTEGER NOT NULL DEFAULT 0,
    equipTribe             INTEGER NOT NULL DEFAULT 0,
    paramBonusType1        INTEGER NOT NULL DEFAULT 0,
    paramBonusValue1       INTEGER NOT NULL DEFAULT 0,
    paramBonusType2        INTEGER NOT NULL DEFAULT 0,
    paramBonusValue2       INTEGER NOT NULL DEFAULT 0,
    paramBonusType3        INTEGER NOT NULL DEFAULT 0,
    paramBonusValue3       INTEGER NOT NULL DEFAULT 0,
    paramBonusType4        INTEGER NOT NULL DEFAULT 0,
    paramBonusValue4       INTEGER NOT NULL DEFAULT 0,
    paramBonusType5        INTEGER NOT NULL DEFAULT 0,
    paramBonusValue5       INTEGER NOT NULL DEFAULT 0,
    paramBonusType6        INTEGER NOT NULL DEFAULT 0,
    paramBonusValue6       INTEGER NOT NULL DEFAULT 0,
    paramBonusType7        INTEGER NOT NULL DEFAULT 0,
    paramBonusValue7       INTEGER NOT NULL DEFAULT 0,
    paramBonusType8        INTEGER NOT NULL DEFAULT 0,
    paramBonusValue8       INTEGER NOT NULL DEFAULT 0,
    paramBonusType9        INTEGER NOT NULL DEFAULT 0,
    paramBonusValue9       INTEGER NOT NULL DEFAULT 0,
    paramBonusType10       INTEGER NOT NULL DEFAULT 0,
    paramBonusValue10      INTEGER NOT NULL DEFAULT 0,
    additionalEffect       INTEGER NOT NULL DEFAULT 0,
    materiaBindPermission  INTEGER NOT NULL DEFAULT 0,
    materializeTable       INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS gamedata_items_graphics (
    catalogID   INTEGER PRIMARY KEY,
    weaponId    INTEGER NOT NULL DEFAULT 0,
    equipmentId INTEGER NOT NULL DEFAULT 0,
    variantId   INTEGER NOT NULL DEFAULT 0,
    colorId     INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS gamedata_items_graphics_extra (
    catalogID          INTEGER PRIMARY KEY,
    offHandWeaponId    INTEGER NOT NULL DEFAULT 0,
    offHandEquipmentId INTEGER NOT NULL DEFAULT 0,
    offHandVarientId   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS gamedata_items_weapon (
    catalogID               INTEGER PRIMARY KEY,
    attack                  INTEGER NOT NULL DEFAULT 0,
    magicAttack             INTEGER NOT NULL DEFAULT 0,
    craftProcessing         INTEGER NOT NULL DEFAULT 0,
    craftMagicProcessing    INTEGER NOT NULL DEFAULT 0,
    harvestPotency          INTEGER NOT NULL DEFAULT 0,
    harvestLimit            INTEGER NOT NULL DEFAULT 0,
    frequency               INTEGER NOT NULL DEFAULT 0,
    rate                    INTEGER NOT NULL DEFAULT 0,
    magicRate               INTEGER NOT NULL DEFAULT 0,
    craftProcessControl     INTEGER NOT NULL DEFAULT 0,
    harvestRate             INTEGER NOT NULL DEFAULT 0,
    critical                INTEGER NOT NULL DEFAULT 0,
    magicCritical           INTEGER NOT NULL DEFAULT 0,
    parry                   INTEGER NOT NULL DEFAULT 0,
    damageAttributeType1    INTEGER NOT NULL DEFAULT 0,
    damageAttributeValue1   REAL NOT NULL DEFAULT 0,
    damageAttributeType2    INTEGER NOT NULL DEFAULT 0,
    damageAttributeValue2   REAL NOT NULL DEFAULT 0,
    damageAttributeType3    INTEGER NOT NULL DEFAULT 0,
    damageAttributeValue3   REAL NOT NULL DEFAULT 0,
    damagePower             INTEGER NOT NULL DEFAULT 0,
    damageInterval          REAL NOT NULL DEFAULT 0,
    ammoVirtualDamagePower  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS gamedata_guildleves (
    id                    INTEGER PRIMARY KEY,
    zoneId                INTEGER DEFAULT 0,
    name                  TEXT DEFAULT '',
    difficulty            INTEGER DEFAULT 0,
    leveType              INTEGER DEFAULT 0,
    rewardExp             INTEGER DEFAULT 0,
    rewardGil             INTEGER DEFAULT 0,
    classType             INTEGER DEFAULT 0,
    location              INTEGER DEFAULT 0,
    factionCreditRequired INTEGER DEFAULT 0,
    level                 INTEGER DEFAULT 0,
    aetheryte             INTEGER DEFAULT 0,
    plateId               INTEGER DEFAULT 0,
    borderId              INTEGER DEFAULT 0,
    objective             INTEGER DEFAULT 0,
    partyRecommended      INTEGER DEFAULT 0,
    targetLocation        INTEGER DEFAULT 0,
    authority             INTEGER DEFAULT 0,
    timeLimit             INTEGER DEFAULT 0,
    skill                 INTEGER DEFAULT 0,
    favorCount            INTEGER DEFAULT 0,
    aimNum1               INTEGER NOT NULL DEFAULT 0,
    aimNum2               INTEGER NOT NULL DEFAULT 0,
    aimNum3               INTEGER NOT NULL DEFAULT 0,
    aimNum4               INTEGER NOT NULL DEFAULT 0,
    item1                 INTEGER NOT NULL DEFAULT 0,
    item2                 INTEGER NOT NULL DEFAULT 0,
    item3                 INTEGER NOT NULL DEFAULT 0,
    item4                 INTEGER NOT NULL DEFAULT 0,
    mob1                  INTEGER NOT NULL DEFAULT 0,
    mob2                  INTEGER NOT NULL DEFAULT 0,
    mob3                  INTEGER NOT NULL DEFAULT 0,
    mob4                  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS gamedata_achievements (
    achievementId  INTEGER PRIMARY KEY,
    name           TEXT NOT NULL,
    packetOffsetId INTEGER NOT NULL DEFAULT 0,
    rewardPoints   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS sessions (
    id         TEXT PRIMARY KEY,
    userId     INTEGER NOT NULL,
    expiration TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS servers (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         TEXT NOT NULL,
    address      TEXT NOT NULL,
    port         INTEGER NOT NULL,
    listPosition INTEGER NOT NULL,
    numchars     INTEGER NOT NULL DEFAULT 0,
    maxchars     INTEGER NOT NULL DEFAULT 5000,
    isActive     INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS reserved_names (
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    userId INTEGER NOT NULL,
    name   TEXT NOT NULL
);

-- Default localhost world row so a fresh database is usable out of the box.
-- Lobby iterates `servers WHERE isActive = true` and world-server looks up
-- its name by `worldId` (`servers.id`), so this seed makes a one-box
-- lobby/world/map rig boot against an empty DB without manual setup.
INSERT OR IGNORE INTO servers (id, name, address, port, listPosition, isActive)
VALUES (1, 'Fernehalwes', '127.0.0.1', 54992, 1, 1);
