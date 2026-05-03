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

//! Commands emitted by Lua userdata methods.
//!
//! Lua scripts run synchronously inside a blocking context; they must not
//! touch async locks. Instead of mutating game state in-place, our userdata
//! types append to a shared `CommandQueue`, and the Map Server's game loop
//! drains it after the script returns.
//!
//! This is the same pattern that avoided deadlocks in the C# port — the
//! difference is just that our queue is typed.

use std::sync::{Arc, Mutex};

/// Every Lua-initiated side effect maps to one of these.
#[derive(Debug, Clone)]
pub enum LuaCommand {
    SendMessage {
        actor_id: u32,
        message_type: u8,
        sender: String,
        text: String,
    },
    EndEvent {
        player_id: u32,
        event_owner: u32,
        event_name: String,
    },
    ChangeState {
        actor_id: u32,
        main_state: u16,
    },
    ChangeMusic {
        player_id: u32,
        music_id: u16,
    },
    PlayAnimation {
        actor_id: u32,
        animation_id: u32,
    },
    SetPos {
        actor_id: u32,
        zone_id: u32,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    },
    GraphicChange {
        actor_id: u32,
        slot: u8,
        graphic_id: u32,
    },
    SpawnActor {
        zone_id: u32,
        actor_class_id: u32,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    },
    /// `currentParty:AddMember(actor_id)` — port of C#
    /// `Party::AddMember`. Appends `member_actor_id` to the
    /// `leader_actor_id` session's transient party-member list and
    /// re-emits the GroupHeader / GroupMembersBegin / X08 / End
    /// sequence so the leader's party-list UI shows the new member.
    /// Used by combat-tutorial scripts to add ally NPCs (Yda +
    /// Papalymo for `SimpleContent30010.lua::onCreate`) so the
    /// allies render as party members.
    PartyAddMember {
        leader_actor_id: u32,
        member_actor_id: u32,
    },
    /// `actor:SetMod(modifier_key, value)` — port of C#
    /// `Chara::SetMod` (`Map Server/Actors/Chara/Chara.cs`). Writes
    /// `value` into the actor's `ModifierMap` keyed by `modifier_key`
    /// (the numeric id from `Modifier`). Used by combat-tutorial /
    /// scripted-event scripts to set durable per-actor flags like
    /// `MinimumHpLock`, movement speed multipliers, etc. Combat math
    /// reads the modifier map on every damage / move event.
    SetActorMod {
        actor_id: u32,
        modifier_key: u32,
        value: i64,
    },
    /// `GetWorldManager().SpawnBattleNpcById(id, contentArea)` — port of
    /// C# `WorldManager.SpawnBattleNpcById` (Map Server/WorldManager.cs:514).
    /// Spawns a BattleNpc by joining the four `server_battlenpc_*` seed
    /// tables (spawn_locations + groups + pools + genus) and materialising
    /// the actor under the parent zone's actor list. The expected
    /// `actor_id` is computed deterministically by the Lua binding so the
    /// caller can chain `actor.actorId` immediately; the apply path uses
    /// the same formula.
    SpawnBattleNpcById {
        bnpc_id: u32,
        parent_zone_id: u32,
        /// Pre-computed actor id the Lua binding handed back to the
        /// caller. Apply path materialises the actor at exactly this
        /// id so any subsequent `director:AddMember(actor)` /
        /// `currentParty:AddMember(actor.actorId)` calls during the
        /// same `onCreate` drain resolve correctly.
        expected_actor_id: u32,
    },
    DespawnActor {
        zone_id: u32,
        actor_id: u32,
    },
    AddExp {
        actor_id: u32,
        class_id: u8,
        exp: i32,
    },
    AddGil {
        actor_id: u32,
        amount: i32,
    },
    /// `player:Die()` — force the actor into the DEAD state (flipping
    /// `current_main_state`, zeroing HP, broadcasting `SetActorState`).
    /// Used by GM commands and by scripted death cutscenes.
    Die {
        actor_id: u32,
    },
    /// `player:Revive()` — bring the actor back from DEAD, restoring
    /// HP/MP to max and broadcasting the state change.
    Revive {
        actor_id: u32,
    },
    AddItem {
        actor_id: u32,
        item_package: u16,
        item_id: u32,
        quantity: i32,
    },
    /// `retainer:GetItemPackage(code):AddItem(id, qty)` — adds a stack
    /// to a retainer's personal inventory (not the bazaar). Routes
    /// through a dedicated DB table keyed by `(retainerId,
    /// serverItemId)` rather than `characters_inventory`, so retainer
    /// and player inventories stay logically disjoint.
    ///
    /// Emitted by [`LuaItemPackage`] when `is_retainer == true` (set
    /// by [`LuaRetainer::GetItemPackage`]). Tier 4 #14 C.
    ///
    /// [`LuaItemPackage`]: crate::lua::userdata::LuaItemPackage
    /// [`LuaRetainer::GetItemPackage`]: crate::lua::userdata::LuaRetainer
    AddItemToRetainer {
        retainer_id: u32,
        item_package: u16,
        item_id: u32,
        quantity: i32,
    },
    /// `levemete:HandInRegionalLeve(player, leve_id)` — the drain-side
    /// trigger for Tier 3 #13 reward payout + Tier 4 #16 C seal
    /// accrual. Routes through `apply_regional_leve_hand_in` which
    /// grants gil / reward-item / (for battlecraft + enlisted
    /// players) GC seals, then clears the leve from the journal.
    /// Silently no-ops when the leve isn't marked completed.
    HandInRegionalLeve {
        player_id: u32,
        leve_id: u32,
    },
    /// `player:AcceptRegionalLeve(leveId, difficulty)` — the
    /// levemete accept-menu counterpart to [`Self::HandInRegionalLeve`].
    /// Adds a quest entry to the journal with `ACCEPTED_FLAG_BIT`
    /// set and `counter2 = difficulty` (clamped 0..=3) so progress
    /// hooks tick correctly against the chosen band. Idempotent on
    /// already-accepted leves. Tier 3 #13 accept-side binding.
    AcceptRegionalLeve {
        player_id: u32,
        leve_id: u32,
        difficulty: u8,
    },
    /// `player:BuyFromRetainer(retainerId, serverItemId)` — drain
    /// trigger for a bazaar purchase. Transactional: debits buyer
    /// gil, credits retainer owner, moves the item stack into the
    /// buyer's NORMAL bag, and removes the listing. Tier 4 #14 D.
    PurchaseRetainerBazaarItem {
        buyer_id: u32,
        retainer_id: u32,
        server_item_id: u64,
    },
    /// `action.TryStatus(source, target, statusId, ...)` — Lua-driven
    /// status-effect application. Tier 1 #2 C. Builds a
    /// [`StatusEffect`] from the given params and applies it via the
    /// target's [`StatusEffectContainer`], draining the resulting
    /// [`StatusOutbox`] so the gain packet + onGain hook fire end-to-
    /// end. Silently no-ops on missing target or full effect table.
    ///
    /// Matches Meteor's inline-constructor shape `new StatusEffect(
    /// ownerId, statusId, magnitude, tickMs, duration, tier)` so
    /// ported scripts can move over without rewiring.
    ///
    /// [`StatusEffect`]: crate::status::StatusEffect
    /// [`StatusEffectContainer`]: crate::status::StatusEffectContainer
    /// [`StatusOutbox`]: crate::status::StatusOutbox
    TryStatus {
        source_actor_id: u32,
        target_actor_id: u32,
        status_id: u32,
        duration_s: u32,
        magnitude: f64,
        tick_ms: u32,
        tier: u8,
    },
    RemoveItem {
        actor_id: u32,
        item_package: u16,
        server_item_id: u64,
    },
    AddQuest {
        player_id: u32,
        quest_id: u32,
    },
    CompleteQuest {
        player_id: u32,
        quest_id: u32,
    },
    AbandonQuest {
        player_id: u32,
        quest_id: u32,
    },
    /// `quest:ClearQuestData()` / `data:ClearData()` — reset every flag +
    /// counter on the live quest.
    QuestClearData {
        player_id: u32,
        quest_id: u32,
    },
    /// `quest:ClearQuestFlags()` — zero the flag bitfield but leave
    /// counters intact. Matches Meteor's `Quest.ClearQuestFlags()`.
    QuestClearFlags {
        player_id: u32,
        quest_id: u32,
    },
    /// `quest:SetQuestFlag(bit)` / `data:SetFlag(bit)`.
    QuestSetFlag {
        player_id: u32,
        quest_id: u32,
        bit: u8,
    },
    /// `data:ClearFlag(bit)`.
    QuestClearFlag {
        player_id: u32,
        quest_id: u32,
        bit: u8,
    },
    /// `data:SetCounter(idx, value)` — value is 0..=65535.
    QuestSetCounter {
        player_id: u32,
        quest_id: u32,
        idx: u8,
        value: u16,
    },
    /// `data:IncCounter(idx)` — wraps at 65_536.
    QuestIncCounter {
        player_id: u32,
        quest_id: u32,
        idx: u8,
    },
    /// `data:DecCounter(idx)` — wraps at 0.
    QuestDecCounter {
        player_id: u32,
        quest_id: u32,
        idx: u8,
    },
    /// `quest:StartSequence(sequence)` — flips Dirty; the dispatcher
    /// fires `onStateChange(player, quest, sequence)` after the current
    /// script finishes so its side effects land after the mutation.
    QuestStartSequence {
        player_id: u32,
        quest_id: u32,
        sequence: u32,
    },
    /// `quest:SetENpc(classId, flagType, isTalkEnabled, isPushEnabled,
    /// isEmoteEnabled, isSpawned)` — register an actively-tracked NPC
    /// for the current sequence. The processor routes this through
    /// `QuestState::add_enpc` and, when the resulting `AddEnpcOutcome`
    /// is `New` or `Updated`, emits `SetEventStatus` + `SetActorQuestGraphic`
    /// packets to the player.
    QuestSetEnpc {
        player_id: u32,
        quest_id: u32,
        actor_class_id: u32,
        quest_flag_type: u8,
        is_talk_enabled: bool,
        is_push_enabled: bool,
        is_emote_enabled: bool,
        is_spawned: bool,
    },
    /// `quest:UpdateENPCs()` — drain `QuestState::old` (ENPCs the new
    /// sequence didn't re-register) and emit clear packets for each.
    /// Meteor's scripts call this at the tail of `onTalk` / `onPush` /
    /// `onKillBNpc` after a mutation that might have changed which
    /// NPCs are quest-active; the engine batches the broadcast so the
    /// script doesn't need to re-emit per-NPC.
    QuestUpdateEnpcs {
        player_id: u32,
        quest_id: u32,
    },
    /// `player:SetQuestComplete(id, flag)` — Meteor's direct-set of
    /// the 2048-bit completion bitfield. Unlike `CompleteQuest`, this
    /// doesn't remove the quest from active slots — GM debug commands
    /// (`!completedQuest`) and cross-quest prerequisites use it to
    /// retroactively mark a completion without running the script's
    /// `onFinish`.
    SetQuestComplete {
        player_id: u32,
        quest_id: u32,
        flag: bool,
    },
    SetHomePoint {
        player_id: u32,
        homepoint: u32,
    },
    /// Mirrors `Player.SetLoginDirector(director)` in C# — used by
    /// `battlenpc.lua` / `player.lua` `onBeginLogin` on the tutorial path.
    /// Flipping this changes the LuaParam shape of the player's
    /// `ActorInstantiate` ScriptBind packet (C# `Player.CreateScriptBindPacket`
    /// branches on `loginInitDirector != null`). Without this command
    /// being fired on tutorial-zone login the 1.23b client stays at Now
    /// Loading because it never sees the "init director attached" variant.
    SetLoginDirector {
        player_id: u32,
        director_actor_id: u32,
    },
    /// `zone:CreateDirector(path, hasContentGroup)` in Lua. The C# version
    /// creates a `Director` actor with `actor_id = (6 << 28) | (zone_actor_id
    /// << 19) | director_local_id` and loads its `.lua` script. We don't
    /// persist any director state cross-session yet; this command just
    /// carries the computed id + classPath back to the Rust side so
    /// `send_zone_in_bundle` can emit the director's 7-packet spawn
    /// sequence and `SetLoginDirector` can reference the right actor id.
    CreateDirector {
        director_actor_id: u32,
        zone_actor_id: u32,
        class_path: String,
    },
    RunEventFunction {
        player_id: u32,
        event_name: String,
        function_name: String,
        args: Vec<LuaCommandArg>,
    },
    KickEvent {
        player_id: u32,
        actor_id: u32,
        trigger: String,
        args: Vec<LuaCommandArg>,
    },
    Warp {
        player_id: u32,
        zone_id: u32,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    },
    DoZoneChange {
        player_id: u32,
        zone_id: u32,
        private_area: Option<String>,
        private_area_type: u32,
        spawn_type: u8,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    },
    /// `area:CreateContentArea(player, classPath, areaName, contentScript,
    /// directorName, ...args)` from Lua. Mirrors C#
    /// `Zone.CreateContentArea` (Map Server/Actors/Area/Zone.cs:170) —
    /// allocates a `PrivateAreaContent` instance under the parent zone,
    /// creates a content director with the given script, and binds the
    /// director's content group to the player. The combat tutorial path
    /// (`man0g0::doContentArea`) calls this with
    /// `(player, "/Area/PrivateArea/Content/PrivateAreaMasterSimpleContent",
    /// "man0g01", "SimpleContent30010", "Quest/QuestDirectorMan0g001")`.
    CreateContentArea {
        player_id: u32,
        parent_zone_id: u32,
        area_class_path: String,
        area_name: String,
        content_script: String,
        director_name: String,
        director_actor_id: u32,
        content_area_actor_id: u32,
    },
    /// `GetWorldManager():DoZoneChangeContent(player, contentArea, x, y,
    /// z, rot, spawnType)` — port of C#
    /// `WorldManager.DoZoneChangeContent` (Map Server/WorldManager.cs:971).
    /// Removes the player from the old area, places them in the content
    /// area, and emits `DeleteAllActors` + `0x00E2` + the standard
    /// zone-in bundle so the client wipes the world and re-renders for
    /// the instance.
    DoZoneChangeContent {
        player_id: u32,
        parent_zone_id: u32,
        area_name: String,
        director_actor_id: u32,
        spawn_type: u8,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    },
    /// `area:ContentFinished()` — flag the content area for cleanup
    /// once the last player exits. The combat tutorial calls this after
    /// the post-fight cinematic finishes.
    ContentFinished {
        parent_zone_id: u32,
        area_name: String,
    },
    /// `player:SpawnMyRetainer(bell, retainerIndex)` — retail
    /// "summon retainer at a bell" path. Resolves `retainer_index`
    /// (1-based) to a `characters_retainers` row, loads the catalog
    /// template, and stashes the runtime snapshot on the Session.
    /// `bell_actor_id` + `bell_position` let the processor compute
    /// the landing position the same way Meteor does (1-unit offset
    /// back toward the player).
    SpawnMyRetainer {
        player_id: u32,
        bell_actor_id: u32,
        bell_position: (f32, f32, f32),
        retainer_index: i32,
    },
    /// `player:DespawnMyRetainer()` — clear the session's spawned
    /// retainer slot. Emits the retainer-despawn packet trio once
    /// the live-spawn path lands; for now the script-visible side
    /// effect is `GetSpawnedRetainer()` returning nil afterward.
    DespawnMyRetainer {
        player_id: u32,
    },
    /// `player:HireRetainer(retainerId)` — confirm flow out of
    /// `PopulaceRetainerManager.lua` after the player names and
    /// confirms a retainer choice. Inserts the `characters_retainers`
    /// row so future `SpawnMyRetainer(bell, idx)` calls find it.
    HireRetainer {
        player_id: u32,
        retainer_id: u32,
    },
    /// `player:DismissMyRetainer(retainerId)` — explicit termination
    /// of a retainer (menu option 10 in `retainer.lua`'s Say Codes).
    /// Deletes the `characters_retainers` row and, if the same
    /// retainer is currently spawned, clears the session snapshot.
    DismissMyRetainer {
        player_id: u32,
        retainer_id: u32,
    },
    /// `retainer:Rename(newName)` — per-character retainer rename
    /// (writes `characters_retainers.customName`, not the shared
    /// `server_retainers.name`). Tier 4 #14 E.
    RenameRetainer {
        player_id: u32,
        retainer_id: u32,
        new_name: String,
    },
    /// `player:SetSleeping()` — snap the player's transform to the
    /// bed of whatever inn room they're currently in (Limsa /
    /// Gridania / Ul'dah — three rooms per inn). Called from
    /// `ObjectBed.lua::onEventStarted` right before the logout /
    /// quit-game RPC so the re-login drops the player onto the
    /// bed rather than wherever they clicked from. Silently no-ops
    /// outside an inn zone.
    SetSleeping {
        player_id: u32,
    },
    /// `player:StartDream(dreamId)` — begin a scripted dream
    /// sequence. Sets the session's `current_dream_id` + emits
    /// `SetPlayerDreamPacket(dreamId, innCode)` so the client
    /// fades to the dream-overlay view. The Hildibrand `etc5*`
    /// quests call this to drive their inn cutscenes.
    StartDream {
        player_id: u32,
        dream_id: u8,
    },
    /// `player:EndDream()` — wake the player up. Clears the
    /// session's dream state and emits `SetPlayerDreamPacket(0, innCode)`
    /// so the client restores the normal view.
    EndDream {
        player_id: u32,
    },
    /// `player:IssueChocobo(appearance, name)` — Grand Company
    /// chocobo license award. Flips `hasChocobo = true`, saves the
    /// appearance id + chosen name to `characters_chocobo` via
    /// `Database::issue_player_chocobo`, and emits
    /// `SetHasChocobo` + `SetChocoboName` to the client.
    IssueChocobo {
        player_id: u32,
        appearance_id: u8,
        name: String,
    },
    /// `player:StartChocoboRental(minutes)` — called from
    /// `PopulaceChocoboLender.lua` on the rental menu choice. Sets
    /// `rentalExpireTime = now + minutes*60`, `rentalMinLeft = minutes`.
    /// The per-tick update then decrements minLeft and auto-dismounts
    /// on expiry.
    StartChocoboRental {
        player_id: u32,
        minutes: u8,
    },
    /// `player:SetMountState(state)` — 0 = on foot, 1 = chocobo,
    /// 2 = goobbue. Flips the helper flag and (when mounted) emits
    /// the `SetCurrentMountChocoboPacket` or `SetCurrentMountGoobbue`
    /// packet broadcast.
    SetMountState {
        player_id: u32,
        state: u8,
    },
    /// `player:SendMountAppearance()` — force a broadcast of the
    /// current mount appearance (used after zone-in or appearance
    /// change). Re-emits `SetCurrentMountChocobo(appearance,
    /// expire, minLeft)` / `SetCurrentMountGoobbue(appearance)` to
    /// every nearby player + self.
    SendMountAppearance {
        player_id: u32,
    },
    /// `player:SetChocoboName(name)` — rename the chocobo without
    /// affecting the rental timer or appearance. Persists via
    /// `Database::change_player_chocobo_name`.
    SetChocoboName {
        player_id: u32,
        name: String,
    },
    /// `player:JoinGC(gc)` — enlist in a Grand Company. Sets
    /// `gc_current` to the GC id (1/2/3) and flips the player's
    /// starter rank to Recruit (127) if not already set. Emits
    /// `SetGrandCompanyPacket` to the client.
    JoinGC {
        player_id: u32,
        gc: u8,
    },
    /// `player:SetGCRank(gc, rank)` — direct rank write, typically
    /// called from the promotion flow. Persists via
    /// `Database::set_gc_rank` + emits `SetGrandCompanyPacket`.
    SetGCRank {
        player_id: u32,
        gc: u8,
        rank: u8,
    },
    /// `player:AddSeals(gc, amount)` — grant seals of a specific GC.
    /// Uses `Database::add_seals` (same transactional upsert as gil).
    /// Zero or negative amounts are treated as "no-op / decay" and
    /// clamped at 0.
    AddSeals {
        player_id: u32,
        gc: u8,
        amount: i32,
    },
    /// `director:EndGuildleve(was_completed)` — script-driven leve
    /// teardown. Called from the `main(thisDirector)` coroutine in
    /// every `directors/Guildleve/*.lua` script when the leve's
    /// objective sequence finishes (`was_completed=true`) or its time
    /// limit elapses (`was_completed=false`). Wires the script-side
    /// trigger to `runtime::director::apply_end_guildleve`, which
    /// looks up the matching `GuildleveDirector` on the zone (decoded
    /// from the actor id's zone-bits), calls its `end_guildleve`
    /// helper into a local `DirectorOutbox`, and drains the resulting
    /// `GuildleveEnded` event through `dispatch_director_event` —
    /// closing the production loop that previously made yesterday's
    /// `award_leve_completion_seals` only fireable from tests.
    EndGuildleve {
        director_actor_id: u32,
        was_completed: bool,
    },
    /// `director:StartGuildleve()` — script-driven leve start. Called
    /// at the top of every `directors/Guildleve/*.lua` `main`
    /// coroutine right after the opening `wait(3)`. Drains a
    /// `GuildleveStarted` (music + start msg + time-limit msg) +
    /// `GuildleveSyncAll` event pair through the dispatcher so the
    /// player sees the leve UI light up.
    StartGuildleve {
        director_actor_id: u32,
    },
    /// `director:AbandonGuildleve()` — fires the abandon-message text
    /// then runs the same teardown as `EndGuildleve(false)`. Used by
    /// `GuildleveWarpPoint.lua` and a couple `Quest/QuestDirector*`
    /// scripts that bail the leve mid-flight.
    AbandonGuildleve {
        director_actor_id: u32,
    },
    /// `director:UpdateAimNumNow(index, value)` — bumps the
    /// `aim_num_now[index]` counter (the "kills remaining" / "items
    /// gathered" tracker the client renders on the leve widget) and
    /// emits a property-update event the dispatcher logs (real
    /// SetActorProperty packet emit is the natural follow-up — the
    /// underlying property-builder pipeline isn't wired through the
    /// dispatcher yet).
    UpdateAimNumNow {
        director_actor_id: u32,
        index: u8,
        value: i8,
    },
    /// `director:UpdateUIState(index, value)` — sibling of
    /// `UpdateAimNumNow` for the `ui_state[index]` slots. Same
    /// dispatcher path, same packet-emit deferral.
    UpdateUiState {
        director_actor_id: u32,
        index: u8,
        value: i8,
    },
    /// `director:UpdateMarkers(index, x, y, z)` — repositions the
    /// leve's per-objective minimap marker. Same dispatcher path as
    /// the aim/ui updaters. Note: script name is plural
    /// (`UpdateMarkers`) but each call moves a single marker — the
    /// `index` arg disambiguates the slot.
    UpdateMarkers {
        director_actor_id: u32,
        index: u8,
        x: f32,
        y: f32,
        z: f32,
    },
    /// `director:SyncAllInfo()` — bulk re-push of every leve property
    /// (aim_num + aim_num_now + ui_state + markers) to every player
    /// member. Called right after `StartGuildleve()` to seed the
    /// client's leve widget.
    SyncAllInfo {
        director_actor_id: u32,
    },
    /// `retainer:AddBazaarItem(itemId, qty, quality, priceGil)` —
    /// add a bazaar listing to the owning retainer. Bazaar
    /// infrastructure foundation: persists to
    /// `characters_retainer_bazaar` via
    /// `Database::add_retainer_bazaar_item` and merges into an
    /// existing same-price stack if one exists. The full
    /// BazaarCheck/Deal/Trade packet family sits on top of this
    /// storage layer; the packets can't ship without the listings
    /// actually persisting across summon/despawn cycles, so this
    /// came first.
    AddRetainerBazaarItem {
        retainer_id: u32,
        item_id: u32,
        quantity: i32,
        quality: u8,
        price_gil: i32,
    },
    /// `director:StartDirector(spawn_immediate)` — the script-level
    /// trigger that kicks off a director's `main(thisDirector)`
    /// coroutine. Mirrors Meteor's `Director.StartDirector`
    /// (`Map Server/Actors/Director/Director.cs:118`) which ends with
    /// `CallLuaScript("main", this, contentGroup)`. Yesterday's
    /// `EndGuildleve` drain is what such a coroutine eventually
    /// reaches; this variant is the missing first step that spawns
    /// the coroutine in the first place.
    ///
    /// `class_path` + `director_name` are pulled from the
    /// `LuaDirectorHandle` at push time so the processor's
    /// `apply_start_director_main` can resolve the script without a
    /// zone re-lookup. `spawn_immediate` is currently advisory —
    /// matches Meteor's arg name; garlemald spawns immediately
    /// regardless.
    StartDirectorMain {
        director_actor_id: u32,
        class_path: String,
        director_name: String,
        spawn_immediate: bool,
    },
    /// `player:PromoteGC(gc)` — atomic seal-spend + rank-bump.
    /// Mirrors the post-confirm tail of Meteor's
    /// `PopulaceCompanyOfficer.lua` flow: `eventDoRankUp` confirms the
    /// promotion choice client-side, the script then asks the server
    /// to actually apply it. Refuses (no DB write, no packet emit)
    /// unless every precondition holds: player is enlisted in `gc`,
    /// current rank has a `next_rank` (i.e. not at or past the 1.23b
    /// `STORY_RANK_CAP = 31`), and the seal balance covers
    /// `gc_promotion_cost(current)`. On success: spends the seal cost,
    /// bumps the per-GC rank field, persists both, and emits
    /// `SetGrandCompanyPacket` so the client sees the new rank
    /// immediately.
    PromoteGC {
        player_id: u32,
        gc: u8,
    },
    /// `quest:OnNotice(player)` — cross-script dispatch that fires the
    /// target quest's `onNotice(player, quest, target)` hook. Used by
    /// `AfterQuestWarpDirector` (and any other director that resumes a
    /// quest mid-flow) to hand control back to the quest's scripted
    /// notice handler. Mirrors Meteor's `Quest.OnNotice(Player, string)`;
    /// the Lua director never supplies the trigger string so the
    /// Rust-side dispatcher fires the hook with an empty extra-args list,
    /// which surfaces as `nil` for `target` in the script — the same
    /// null-string the C# variant produces when called without a trigger.
    QuestOnNotice {
        player_id: u32,
        quest_id: u32,
    },
    /// `player:Logout()` — soft logout. Mirrors C# `Player.Logout`
    /// (`Map Server/Actors/Chara/Player/Player.cs:861`): emits
    /// `LogoutPacket` (opcode `0x000E`) to the owning session so the
    /// client returns to character select. Called from `ObjectBed.lua`
    /// (bed menu choice 3 = "Sleep — Stay logged in") and
    /// `LogoutCommand.lua` (chat menu choice 2). Sibling to
    /// [`QuitGame`] which flips the client all the way back to
    /// the launcher instead of character select.
    Logout {
        player_id: u32,
    },
    /// `player:QuitGame()` — hard exit to title. Mirrors C#
    /// `Player.QuitGame` (`Map Server/Actors/Chara/Player/Player.cs:869`):
    /// emits `QuitPacket` (opcode `0x0011`) to the owning session,
    /// which closes the client process. Called from `ObjectBed.lua`
    /// (bed menu choice 2 = "Sleep — Quit") and `LogoutCommand.lua`
    /// (chat menu choice 1).
    QuitGame {
        player_id: u32,
    },
    LogError(String),
}

/// Value-equivalent of a Lua script parameter. Matches `common::LuaParam` but
/// lives here because the command queue predates wire-format concerns.
#[derive(Debug, Clone)]
pub enum LuaCommandArg {
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Bool(bool),
    Nil,
    ActorId(u32),
}

/// Shared-ownership queue. Every userdata instance holds an `Arc<Mutex<…>>`
/// into the queue for the surrounding script invocation.
#[derive(Debug, Default)]
pub struct CommandQueue {
    commands: Vec<LuaCommand>,
}

impl CommandQueue {
    pub fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::default()))
    }

    pub fn push(queue: &Arc<Mutex<Self>>, command: LuaCommand) {
        if let Ok(mut inner) = queue.lock() {
            inner.commands.push(command);
        }
    }

    pub fn drain(queue: &Arc<Mutex<Self>>) -> Vec<LuaCommand> {
        match queue.lock() {
            Ok(mut inner) => std::mem::take(&mut inner.commands),
            Err(_) => Vec::new(),
        }
    }

    pub fn len(queue: &Arc<Mutex<Self>>) -> usize {
        queue.lock().map(|q| q.commands.len()).unwrap_or(0)
    }
}
