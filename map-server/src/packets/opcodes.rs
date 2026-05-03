// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

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

// Mass Delete Actor "(xN)" family. The wiki's `(xN)` labels are
// HEX (x10 = 16, x20 = 32, x40 = 64), confirmed by the captured
// retail bytes:
//   • 0x0009 OUT body is 80 bytes = 16×u32 actor + 16-byte pad
//   • 0x000A OUT body is 160 bytes = 32×u32 actor + 32-byte pad
// Garlemald's existing inventory `_X08/_X16/_X32/_X64` constants
// use decimal counts; the names below follow the same convention,
// but each entry calls out the wiki's hex label so cross-reference
// stays clean.
//
// Sequence semantics (per wiki + captures): 0x0006 Start opens the
// frame, 0x0008/0x0009/0x000A/0x000B body packets list actors to
// EXEMPT from a world wipe, 0x0007 End fires the actual delete
// against everyone NOT in the exempt list. Sending 0x0007 alone
// (which `build_delete_all_actors` does today) wipes everything.

/// Wiki: "Mass Delete Actor Start" (server→client). 8-byte zero
/// body. Opens a Mass Delete Actor sequence — body packets list
/// actors to exempt; the Mass Delete Actor End packet (0x0007)
/// commits the delete. Same opcode value as `OP_RX_LANGUAGE_CODE`
/// (0x0006) — direction disambiguates.
pub const OP_MASS_DELETE_ACTOR_START: u16 = 0x0006;
/// Wiki: "Mass Delete Actor Body (x10)" — `x10` is HEX (16 actor
/// ids per packet). Body = 80 bytes (16×u32 actor + 16 pad).
/// Retail uses it during big zone transitions
/// (`teleport_to_camp_tranquil`, `moving_around_gridania`).
pub const OP_MASS_DELETE_ACTOR_X16: u16 = 0x0009;
/// Wiki: "Mass Delete Actor Body (x20)" — `x20` is HEX (32 actor
/// ids per packet). Body = 160 bytes (32×u32 actor + 32 pad).
/// Seen 2× in the survey (`move_out_of_room`,
/// `moving_around_gridania`).
pub const OP_MASS_DELETE_ACTOR_X32: u16 = 0x000A;
/// Wiki: "Mass Delete Actor Body (x40)" — `x40` is HEX (64 actor
/// ids per packet). Body = 320 bytes (64×u32 actor + 64 pad).
/// Not observed in the 56-capture survey, but defined for symmetry.
pub const OP_MASS_DELETE_ACTOR_X64: u16 = 0x000B;
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
// World↔map-server group coordination (0x1020-0x1032). Used when the map
// server asks the world server to mutate shared party/linkshell state.
// ---------------------------------------------------------------------------
pub const OP_WORLD_PARTY_MODIFY: u16 = 0x1020;
pub const OP_WORLD_PARTY_LEAVE: u16 = 0x1021;
pub const OP_WORLD_PARTY_INVITE: u16 = 0x1022;
pub const OP_WORLD_GROUP_INVITE_RESULT: u16 = 0x1023;
pub const OP_WORLD_CREATE_LINKSHELL: u16 = 0x1025;
pub const OP_WORLD_MODIFY_LINKSHELL: u16 = 0x1026;
pub const OP_WORLD_DELETE_LINKSHELL: u16 = 0x1027;
pub const OP_WORLD_LINKSHELL_CHANGE: u16 = 0x1028;
pub const OP_WORLD_LINKSHELL_INVITE: u16 = 0x1029;
pub const OP_WORLD_LINKSHELL_INVITE_CANCEL: u16 = 0x1030;
pub const OP_WORLD_LINKSHELL_LEAVE: u16 = 0x1031;
pub const OP_WORLD_LINKSHELL_RANK_CHANGE: u16 = 0x1032;

// World → map-server result/error frames received on the same channel.
pub const OP_WORLD_PARTY_SYNC: u16 = 0x1010;
pub const OP_WORLD_LINKSHELL_RESULT: u16 = 0x1011;
pub const OP_WORLD_ERROR: u16 = 0x1FFF;

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
/// LanguageCode (0x0006) — fired by the client once it's safe to receive
/// world-spawn packets. C# `Map/PacketProcessor.cs` case 0x0006 uses this as
/// the deferred trigger for `DoZoneIn(actor, isLogin=true, 0x1)` plus the
/// `onBeginLogin` / `onLogin` Lua hooks.
pub const OP_RX_LANGUAGE_CODE: u16 = 0x0006;
pub const OP_RX_EVENT_START: u16 = 0x012D;
pub const OP_RX_EVENT_UPDATE: u16 = 0x012E;
/// Wiki: "Data Request" (client→server). Same opcode as outbound
/// `OP_KICK_EVENT`; collision distinguished by direction. The client
/// sends one of these to request a GAM-property refresh by path
/// (captured payload format: u32 target_actor_id + ASCII property
/// path null-padded to 20 bytes + 8 bytes of variable trailing data).
/// Retail emits 44 of these per session — ignored by the dispatcher
/// before this audit.
pub const OP_RX_DATA_REQUEST: u16 = 0x012F;
/// Wiki: "Group Created" (client→server). Same opcode as outbound
/// `OP_GENERIC_DATA`. Client sends one when it first observes a
/// monster group / actor and wants the server to register that
/// actor's event handlers — captured payload is u64 actor/group id
/// (synthetic 0x2680… prefix for monster groups) + ASCII event
/// name (`/_init`) padded to 16 bytes + 16 bytes reserved. Retail
/// fires 270 of these per session; dropped before this audit.
pub const OP_RX_GROUP_CREATED: u16 = 0x0133;
/// Wiki: "Target Locked" (client→server). The 1.x client sends this
/// when the player presses target-lock on an actor. Const already
/// defined (`OP_RX_LOCK_TARGET = 0x00CC` below) but **not** in
/// `handle_game_message`'s dispatch table. Retail: 66 events.
/// Wiki: "Target Selected" (client→server). Sent when the player
/// soft-targets an actor. Const `OP_RX_SET_TARGET = 0x00CD` exists
/// but isn't dispatched. Retail: 118 events.
/// Wiki: "Unknown 0x007" — RX_ZONE_IN_COMPLETE per garlemald's
/// existing const. Already defined as `OP_RX_ZONE_IN_COMPLETE` but
/// not in dispatch. Retail: 24 events.

/// Chat. Client sends at opcode 0x0003 (collision with send's
/// `OP_SEND_MESSAGE_PUBLIC` — distinguished by direction).
pub const OP_RX_CHAT_MESSAGE: u16 = 0x0003;

/// Recruitment (party finder).
pub const OP_RX_START_RECRUITING: u16 = 0x01C3;
pub const OP_RX_END_RECRUITING: u16 = 0x01C4;
pub const OP_RX_RECRUITER_STATE: u16 = 0x01C5;
pub const OP_RX_RECRUITING_ACCEPTED: u16 = 0x01C6;
pub const OP_RX_RECRUITING_SEARCH: u16 = 0x01C7;
pub const OP_RX_RECRUITING_DETAILS: u16 = 0x01C8;

/// Social (blacklist / friendlist).
pub const OP_RX_BLACKLIST_ADD: u16 = 0x01C9;
pub const OP_RX_BLACKLIST_REMOVE: u16 = 0x01CA;
pub const OP_RX_BLACKLIST_REQUEST: u16 = 0x01CB;
pub const OP_RX_FRIENDLIST_ADD: u16 = 0x01CC;
pub const OP_RX_FRIENDLIST_REMOVE: u16 = 0x01CD;
pub const OP_RX_FRIENDLIST_REQUEST: u16 = 0x01CE;
pub const OP_RX_FRIEND_STATUS: u16 = 0x01CF;

/// Achievement progress query.
pub const OP_RX_ACHIEVEMENT_PROGRESS: u16 = 0x0135;
/// Item-package query (the C# 0x0131 path — used for retainer item
/// listings + bazaar).
pub const OP_RX_ITEM_PACKAGE_REQUEST: u16 = 0x0131;

/// Support desk.
pub const OP_RX_FAQ_LIST_REQUEST: u16 = 0x01D0;
pub const OP_RX_FAQ_BODY_REQUEST: u16 = 0x01D1;
pub const OP_RX_SUPPORT_ISSUE_REQUEST: u16 = 0x01D2;
pub const OP_RX_GM_TICKET_STATE: u16 = 0x01D3;
pub const OP_RX_GM_TICKET_BODY: u16 = 0x01D4;
pub const OP_RX_GM_TICKET_SEND: u16 = 0x01D5;
pub const OP_RX_GM_TICKET_END: u16 = 0x01D6;
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
/// Wiki: "Reset Head". Resets a previously-set head/eye-tracking
/// orientation set via 0x00DB (Set Head to Actor) or 0x00DC (Set Head
/// to Position). Retail emits 43× across combat/event captures.
pub const OP_RESET_HEAD: u16 = 0x00DE;
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

// "Text Sheet Message" family — server text-id messages routed to the
// client's chat / event log. Three sender variants × five payload-size
// tiers each. Retail captures use the No-Source-Actor path heavily for
// system messages ("You harvest…", "Quest accepted", etc.); garlemald
// today only emits the Source-Actor variants (0x0157-0x015B).
pub const OP_TEXT_SHEET_CUSTOM_SENDER_X48: u16 = 0x015C;
pub const OP_TEXT_SHEET_CUSTOM_SENDER_X58: u16 = 0x015D;
pub const OP_TEXT_SHEET_CUSTOM_SENDER_X68: u16 = 0x015E;
pub const OP_TEXT_SHEET_CUSTOM_SENDER_X78: u16 = 0x015F;
pub const OP_TEXT_SHEET_CUSTOM_SENDER_X98: u16 = 0x0160;
/// Wiki: "Text Sheet Message (DispId Sender) (30b)". Retail: 4× in
/// `accept_leve.pcapng`. Sender is a display-id (e.g. a leve / quest
/// title-card) rather than a runtime actor id.
pub const OP_TEXT_SHEET_DISPID_SENDER_X30: u16 = 0x0161;
pub const OP_TEXT_SHEET_DISPID_SENDER_X38: u16 = 0x0162;
pub const OP_TEXT_SHEET_DISPID_SENDER_X40: u16 = 0x0163;
pub const OP_TEXT_SHEET_DISPID_SENDER_X50: u16 = 0x0164;
pub const OP_TEXT_SHEET_DISPID_SENDER_X60: u16 = 0x0165;
/// Wiki: "Text Sheet Message (No Source Actor) (28b)". Retail: 34× —
/// `checkbed`, `gather_wood`, `local_leve_complete`, etc. Smallest of
/// the no-actor variants (single text-id + minimal params).
pub const OP_TEXT_SHEET_NO_ACTOR_X28: u16 = 0x0166;
/// Wiki: "Text Sheet Message (No Source Actor) (38b)". Retail: 78× —
/// the most-emitted system message variant in the survey.
pub const OP_TEXT_SHEET_NO_ACTOR_X38: u16 = 0x0167;
/// Wiki: "Text Sheet Message (No Source Actor) (38b)" — second 38-byte
/// variant. Retail: 41×.
pub const OP_TEXT_SHEET_NO_ACTOR_X38_ALT: u16 = 0x0168;
/// Wiki: "Text Sheet Message (No Source Actor) (48b)". Retail: 51×.
pub const OP_TEXT_SHEET_NO_ACTOR_X48: u16 = 0x0169;
pub const OP_TEXT_SHEET_NO_ACTOR_X68: u16 = 0x016A;

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
/// Wiki: "Set Occupancy Group (DOUBLE CHECK!)". Retail emits 44× in
/// combat / quest captures. Likely manages duty/instance occupancy
/// state — exact payload unconfirmed.
pub const OP_SET_OCCUPANCY_GROUP: u16 = 0x0187;
pub const OP_CREATE_NAMED_GROUP: u16 = 0x0188;
pub const OP_CREATE_NAMED_GROUP_MULTIPLE: u16 = 0x0189;
/// Wiki: "Set Active Linkshell". Retail: 1× in `login.pcapng`.
/// Implementation already exists at
/// `world-server/src/packets/send.rs::build_set_active_linkshell`
/// (no const previously — added here so the call site is greppable).
pub const OP_SET_ACTIVE_LINKSHELL: u16 = 0x018A;
/// Wiki: "Set Group LayoutID". Retail: 287× across combat captures.
/// Per-group UI layout id (party-list ordering, cross-world group
/// formatting). Garlemald has no builder.
pub const OP_SET_GROUP_LAYOUT_ID: u16 = 0x018B;
/// Wiki: "Party Map Marker Update (x16, variable)". Retail: 592×
/// across 38 captures — the most-emitted of the new-to-garlemald set.
/// Variable-length party-map-marker chunk (icons on the world map for
/// party members).
pub const OP_PARTY_MAP_MARKER_UPDATE: u16 = 0x018D;

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
// Item-modifier mass emission (0x018F-0x0191). Retail emits these in a
// begin / body* / end framing per inventory bag during the initial bag
// snapshot. The body packet (0x0190) is the highest-volume opcode in
// the entire 56-capture survey at **5,569 emissions**, so this is one
// of the larger conformance gaps.
// ---------------------------------------------------------------------------
/// Wiki: "Mass Set Item Modifier Begin". Frame-start marker for a
/// burst of 0x0190 bodies.
pub const OP_MASS_SET_ITEM_MODIFIER_BEGIN: u16 = 0x018F;
/// Wiki: "Mass Set Item Modifier" — per-slot modifier emission inside
/// a begin/end frame. Carries durability / spirit-bind / materia /
/// stack metadata for one item. Retail: 5569×.
pub const OP_MASS_SET_ITEM_MODIFIER: u16 = 0x0190;
/// Wiki: "Mass Set Item Modifier End". Frame-end marker.
pub const OP_MASS_SET_ITEM_MODIFIER_END: u16 = 0x0191;
/// Wiki: "Send Addiction Limit Message". Play-time / parental-control
/// message; observed once in `login.pcapng`.
pub const OP_SEND_ADDICTION_LIMIT_MESSAGE: u16 = 0x0192;
/// Wiki: "Stops control (0x14) and starts (0x15)". Movement-control
/// gate emitted around cutscene / interactive-event boundaries.
/// Retail: 9× across `gridania_to_coerthas`, `move_out_of_room`,
/// `party_battle_leve`, and others.
pub const OP_SET_CONTROL_STATE: u16 = 0x0193;

// ---------------------------------------------------------------------------
// Player state (0x0194–0x01AF range)
// ---------------------------------------------------------------------------
pub const OP_SET_GRAND_COMPANY: u16 = 0x0194;
/// Wiki: "Set Emnity Indicator" (sic). Retail: 149× across
/// `combat_autoattack`, `combat_skills`, `party_battle_leve`,
/// `war_quest_update2`. Per-mob enmity / hate UI indicator.
pub const OP_SET_ENMITY_INDICATOR: u16 = 0x0195;
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
