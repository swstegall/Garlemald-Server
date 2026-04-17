//! Opcode registry. Names match the C# Packets/Send and Packets/Receive
//! hierarchy to simplify cross-referencing.
//!
//! Values come straight from the C# `OPCODE` constants — grouped below so
//! the processor can pick out the right response per category.
#![allow(dead_code)]

// ---------------------------------------------------------------------------
// Connection / handshake (raw client frames; NOT game-message subpackets).
// ---------------------------------------------------------------------------
pub const OP_PONG_RESPONSE: u16 = 0x0001;
pub const OP_HANDSHAKE_RESPONSE: u16 = 0x0002;
pub const OP_SEND_MESSAGE_PUBLIC: u16 = 0x0003;
pub const OP_SET_MAP: u16 = 0x0005;
pub const OP_DELETE_ALL_ACTORS: u16 = 0x0007;
pub const OP_PONG: u16 = 0x0008;
pub const OP_SET_MUSIC: u16 = 0x000C;
pub const OP_SET_WEATHER: u16 = 0x000D;
pub const OP_LOGOUT: u16 = 0x000E;
pub const OP_0XF_PACKET: u16 = 0x000F;
pub const OP_SET_DALAMUD: u16 = 0x0010;
pub const OP_QUIT: u16 = 0x0011;

// ---------------------------------------------------------------------------
// World↔map-server session control (>= 0x1000 subpackets).
// ---------------------------------------------------------------------------
pub const OP_SESSION_BEGIN: u16 = 0x1000;
pub const OP_SESSION_END: u16 = 0x1001;
pub const OP_WORLD_ZONE_CHANGE_REQUEST: u16 = 0x1002;

// ---------------------------------------------------------------------------
// Actor lifecycle (game-message opcode in the gamemessage header).
// ---------------------------------------------------------------------------
/// Inbound opcode for `UpdatePlayerPositionPacket` — shares its wire id
/// with the outbound `OP_ADD_ACTOR` (direction disambiguates).
pub const OP_RX_UPDATE_PLAYER_POSITION: u16 = 0x00CA;
pub const OP_ADD_ACTOR: u16 = 0x00CA;
pub const OP_RX_SET_TARGET: u16 = 0x00CD;
pub const OP_RX_LOCK_TARGET: u16 = 0x00CC;
pub const OP_RX_ZONE_IN_COMPLETE: u16 = 0x0007;
pub const OP_RX_EVENT_START: u16 = 0x012D;
pub const OP_RX_EVENT_UPDATE: u16 = 0x012E;
pub const OP_REMOVE_ACTOR: u16 = 0x00CB;
pub const OP_ACTOR_INSTANTIATE: u16 = 0x00CC;
pub const OP_SET_ACTOR_POSITION: u16 = 0x00CE;
pub const OP_MOVE_ACTOR_TO_POSITION: u16 = 0x00CF;
pub const OP_SET_ACTOR_SPEED: u16 = 0x00D0;
pub const OP_SET_ACTOR_TARGET_ANIMATED: u16 = 0x00D3;
pub const OP_SET_ACTOR_APPEARANCE: u16 = 0x00D6;
pub const OP_SET_ACTOR_BG_PROPERTIES: u16 = 0x00D8;
pub const OP_PLAY_BG_ANIMATION: u16 = 0x00D9;
pub const OP_PLAY_ANIMATION_ON_ACTOR: u16 = 0x00DA;
pub const OP_SET_ACTOR_TARGET: u16 = 0x00DB;
pub const OP_ACTOR_DO_EMOTE: u16 = 0x00E1;
pub const OP_0XE2_PACKET: u16 = 0x00E2;
pub const OP_ACTOR_SPECIAL_GRAPHIC: u16 = 0x00E3;
pub const OP_START_COUNTDOWN: u16 = 0x00E5;

pub const OP_SET_ACTOR_NAME: u16 = 0x013D;
pub const OP_SET_ACTOR_STATE: u16 = 0x0134;
pub const OP_SET_EVENT_STATUS: u16 = 0x0136;
pub const OP_SET_ACTOR_PROPERTY: u16 = 0x0137;
pub const OP_SET_ACTOR_SUB_STATE: u16 = 0x0144;
pub const OP_SET_ACTOR_ICON: u16 = 0x0145;
pub const OP_INVENTORY_SET_BEGIN: u16 = 0x0146;
pub const OP_INVENTORY_SET_END: u16 = 0x0147;
pub const OP_INVENTORY_LIST_X01: u16 = 0x0148;
pub const OP_INVENTORY_LIST_X08: u16 = 0x0149;
pub const OP_INVENTORY_LIST_X16: u16 = 0x014A;
pub const OP_INVENTORY_LIST_X32: u16 = 0x014B;
pub const OP_INVENTORY_LIST_X64: u16 = 0x014C;
pub const OP_LINKED_ITEM_LIST_X01: u16 = 0x014D;
pub const OP_LINKED_ITEM_LIST_X08: u16 = 0x014E;
pub const OP_LINKED_ITEM_LIST_X16: u16 = 0x014F;
pub const OP_LINKED_ITEM_LIST_X32: u16 = 0x0150;
pub const OP_LINKED_ITEM_LIST_X64: u16 = 0x0151;
pub const OP_INVENTORY_REMOVE_X01: u16 = 0x0152;
pub const OP_INVENTORY_REMOVE_X08: u16 = 0x0153;
pub const OP_INVENTORY_REMOVE_X16: u16 = 0x0154;
pub const OP_INVENTORY_REMOVE_X32: u16 = 0x0155;
pub const OP_INVENTORY_REMOVE_X64: u16 = 0x0156;

pub const OP_GAME_MESSAGE_ACTOR1: u16 = 0x0157;
pub const OP_GAME_MESSAGE_ACTOR2: u16 = 0x0158;
pub const OP_GAME_MESSAGE_ACTOR3: u16 = 0x0159;
pub const OP_GAME_MESSAGE_ACTOR4: u16 = 0x015A;
pub const OP_GAME_MESSAGE_ACTOR5: u16 = 0x015B;

pub const OP_SET_ACTOR_IS_ZONING: u16 = 0x017B;
pub const OP_SET_ACTOR_STATUS: u16 = 0x0177;
pub const OP_SET_ACTOR_STATUS_ALL: u16 = 0x0179;
pub const OP_SYNCH_GROUP_WORK_VALUES: u16 = 0x017A;
pub const OP_GROUP_HEADER: u16 = 0x017C;
pub const OP_GROUP_MEMBERS_BEGIN: u16 = 0x017D;
pub const OP_GROUP_MEMBERS_END: u16 = 0x017E;
pub const OP_GROUP_MEMBERS_X08: u16 = 0x017F;
pub const OP_GROUP_MEMBERS_X16: u16 = 0x0180;
pub const OP_GROUP_MEMBERS_X32: u16 = 0x0181;
pub const OP_GROUP_MEMBERS_X64: u16 = 0x0182;
pub const OP_CONTENT_MEMBERS_X08: u16 = 0x0183;
pub const OP_CONTENT_MEMBERS_X16: u16 = 0x0184;
pub const OP_CONTENT_MEMBERS_X32: u16 = 0x0185;
pub const OP_CONTENT_MEMBERS_X64: u16 = 0x0186;
pub const OP_CREATE_NAMED_GROUP: u16 = 0x0188;
pub const OP_CREATE_NAMED_GROUP_MULTIPLE: u16 = 0x0189;

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------
pub const OP_SET_TALK_EVENT_CONDITION: u16 = 0x012E;
pub const OP_KICK_EVENT: u16 = 0x012F;
pub const OP_RUN_EVENT_FUNCTION: u16 = 0x0130;
pub const OP_END_EVENT: u16 = 0x0131;
pub const OP_0X132_PACKET: u16 = 0x0132;
pub const OP_GENERIC_DATA: u16 = 0x0133;
pub const OP_DELETE_GROUP: u16 = 0x0143;
pub const OP_SET_NOTICE_EVENT_CONDITION: u16 = 0x016B;
pub const OP_SET_EMOTE_EVENT_CONDITION: u16 = 0x016C;
pub const OP_INVENTORY_BEGIN_CHANGE: u16 = 0x016D;
pub const OP_INVENTORY_END_CHANGE: u16 = 0x016E;
pub const OP_SET_PUSH_CIRCLE_EVENT_CONDITION: u16 = 0x016F;
pub const OP_SET_PUSH_FAN_EVENT_CONDITION: u16 = 0x0170;
pub const OP_SET_PUSH_BOX_EVENT_CONDITION: u16 = 0x0175;

// ---------------------------------------------------------------------------
// Battle
// ---------------------------------------------------------------------------
pub const OP_COMMAND_RESULT_X01: u16 = 0x0139;
pub const OP_COMMAND_RESULT_X10: u16 = 0x013A;
pub const OP_COMMAND_RESULT_X18: u16 = 0x013B;
pub const OP_COMMAND_RESULT_X00: u16 = 0x013C;
pub const OP_BATTLE_ACTION_X10: u16 = 0x013A;
pub const OP_BATTLE_ACTION_X18: u16 = 0x013B;

// ---------------------------------------------------------------------------
// Chat / system messages
// ---------------------------------------------------------------------------
pub const OP_SEND_MESSAGE: u16 = 0x00CA;
pub const OP_GAME_MESSAGE: u16 = 0x01FD;

// ---------------------------------------------------------------------------
// Player state (0x0190–0x01AF range)
// ---------------------------------------------------------------------------
pub const OP_SET_GRAND_COMPANY: u16 = 0x0194;
pub const OP_SET_SPECIAL_EVENT_WORK: u16 = 0x0196;
pub const OP_SET_CURRENT_MOUNT_CHOCOBO: u16 = 0x0197;
pub const OP_SET_CHOCOBO_NAME: u16 = 0x0198;
pub const OP_SET_HAS_CHOCOBO: u16 = 0x0199;
pub const OP_SET_COMPLETED_ACHIEVEMENTS: u16 = 0x019A;
pub const OP_SET_LATEST_ACHIEVEMENTS: u16 = 0x019B;
pub const OP_SET_ACHIEVEMENT_POINTS: u16 = 0x019C;
pub const OP_SET_PLAYER_TITLE: u16 = 0x019D;
pub const OP_ACHIEVEMENT_EARNED: u16 = 0x019E;
pub const OP_SEND_ACHIEVEMENT_RATE: u16 = 0x019F;
pub const OP_SET_CURRENT_MOUNT_GOOBBUE: u16 = 0x01A0;
pub const OP_SET_HAS_GOOBBUE: u16 = 0x01A1;
pub const OP_SET_CUTSCENE_BOOK: u16 = 0x01A3;
pub const OP_SET_CURRENT_JOB: u16 = 0x01A4;
pub const OP_SET_PLAYER_ITEM_STORAGE: u16 = 0x01A5;
pub const OP_SET_PLAYER_DREAM: u16 = 0x01A7;

// ---------------------------------------------------------------------------
// Social / friends / blacklist
// ---------------------------------------------------------------------------
pub const OP_BLACKLIST_ADDED: u16 = 0x01C9;
pub const OP_BLACKLIST_REMOVED: u16 = 0x01CA;
pub const OP_SEND_BLACKLIST: u16 = 0x01CB;
pub const OP_FRIENDLIST_ADDED: u16 = 0x01CC;
pub const OP_FRIENDLIST_REMOVED: u16 = 0x01CD;
pub const OP_SEND_FRIENDLIST: u16 = 0x01CE;
pub const OP_FRIEND_STATUS: u16 = 0x01CF;

// ---------------------------------------------------------------------------
// Support desk
// ---------------------------------------------------------------------------
pub const OP_FAQ_LIST_RESPONSE: u16 = 0x01D0;
pub const OP_FAQ_BODY_RESPONSE: u16 = 0x01D1;
pub const OP_ISSUE_LIST_RESPONSE: u16 = 0x01D2;
pub const OP_START_GM_TICKET: u16 = 0x01D3;
pub const OP_GM_TICKET: u16 = 0x01D4;
pub const OP_GM_TICKET_SENT_RESPONSE: u16 = 0x01D5;
pub const OP_END_GM_TICKET: u16 = 0x01D6;

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------
pub const OP_ITEM_SEARCH_RESULTS_BEGIN: u16 = 0x01D7;
pub const OP_ITEM_SEARCH_RESULTS_BODY: u16 = 0x01D8;
pub const OP_ITEM_SEARCH_RESULTS_END: u16 = 0x01D9;
pub const OP_RETAINER_RESULT_END: u16 = 0x01DA;
pub const OP_RETAINER_RESULT_BODY: u16 = 0x01DB;
pub const OP_RETAINER_RESULT_UPDATE: u16 = 0x01DC;
pub const OP_RETAINER_SEARCH_HISTORY: u16 = 0x01DD;
pub const OP_PLAYER_SEARCH_INFO_RESULT: u16 = 0x01DF;
pub const OP_PLAYER_SEARCH_COMMENT_RESULT: u16 = 0x01E0;
pub const OP_ITEM_SEARCH_CLOSE: u16 = 0x01E1;

// ---------------------------------------------------------------------------
// Recruitment
// ---------------------------------------------------------------------------
pub const OP_START_RECRUITING_RESPONSE: u16 = 0x01C3;
pub const OP_END_RECRUITMENT: u16 = 0x01C4;
pub const OP_RECRUITER_STATE: u16 = 0x01C5;
pub const OP_CURRENT_RECRUITMENT_DETAILS: u16 = 0x01C8;
