# Post-warp respawn fix — analysis + proposed implementation

> Drafted 2026-05-04 from meteor-decomp's Phase 7 cinematic-receiver
> findings. Pre-work for the man0g0 SimpleContent cinematic hang
> that stuck at "Now Loading" after talking to Yda the second time.

## The bug, in one paragraph

When `apply_do_zone_change_content` warps a player to a content
area, it dispatches `DeleteAllActors` (which wipes the client's
actor list) followed by `send_zone_in_bundle` (which re-spawns
ONLY the player + login director — NOT the BattleNpcs / other
content-area actors). The trailing `KickEvent` packet then targets
a director that the client knows about (because the bundle
prepended its spawn packets), but **any RunEventFunction packet
that targets a BattleNpc spawned by the content script's `onCreate`
falls into the empty post-wipe registry and silently drops** —
because the client checks `actor[+0x5c] != 0` (kick gate) and
`actor[+0x7d] != 0` (run-event gate) on the looked-up actor, and
the actor isn't there to gate against. Pmeteor's equivalent code
calls `playerSession.UpdateInstance(aroundMe, true)` AFTER the
wipe, which iterates all area actors and re-broadcasts spawn
packets per actor; garlemald skips this step.

Reference: meteor-decomp docs
- `meteor-decomp/docs/event_kick_receiver_decomp.md` (`+0x5c` gate)
- `meteor-decomp/docs/event_run_event_function_receiver_decomp.md` (`+0x7d` gate)

## Current code path — `apply_do_zone_change_content`

Located at `map-server/src/processor.rs:2383`. Current sequence:

```rust
1. Update character position           // chara writes
2. Update session destination          // session.destination_*
3. send DeleteAllActors                 // ← THE WIPE
   send 0x00E2(0x10) marker
4. send_zone_in_bundle                  // re-spawns PLAYER + login director only
5. fire onZoneIn content-script hook    // may register more spawns via Lua
6. log + done
```

Step 3 wipes the client's actor list. Step 4 re-establishes only
the player and the login director (the latter via spawn packets
prepended in `send_zone_in_bundle`). **Anything else that was in
the destination zone — BattleNpcs spawned by `onCreate`,
NPCs/SNpcs/MapObjs from the destination zone's seed data — is
silently lost** until the next refresh.

## REVISED fix (2026-05-04, after pmeteor cross-reference)

**Important refinement after reading pmeteor's
`Session.UpdateInstance(true)`:** pmeteor emits **THREE packet
groups per re-spawned actor**, not just the spawn bundle:

```
For each actor in surrounding 50-unit area:
  1. GetSpawnPackets(player, 1)        -- 8 packets
       AddActor(flag=8) + EventConditionPackets +
       SetSpeed + SetSpawnPosition + SetName + SetState +
       SetIsZoning + ActorInstantiate

  2. GetInitPackets()                   -- 1 packet
       SetActorPropertyPacket("/_init") with 3 byte flags
       (0xE14B0CA8, 0x2138FD71, 0xFBFBCFB1) all = 1

  3. GetSetEventStatusPackets()         -- N packets (per condition)
       SetEventStatus(actorId, enabled, type, conditionName)
       for each talk/notice/emote/push condition
```

Pmeteor source: `Map Server/Actors/Actor.cs:257-360` and
`Map Server/DataObjects/Session.cs:112-170`.

**Critical insight:** garlemald's existing `spawn_bundle_fanout`
emits ONLY group 1 (and even then with `flag=0` instead of
pmeteor's `flag=8` for non-self spawns). Groups 2 and 3 are
missing. Crucially, **`GetSetEventStatusPackets()` is what
populates the actor's event-condition list on the client side
— without those, `KickEvent` and `RunEventFunction` have no
condition to dispatch against and silently drop**, even though
the actor exists in the registry.

Per meteor-decomp's
`reference_meteor_decomp_actor_rtti.md`, the `+0x5c` byte the
kick receiver checks is most likely set as a side effect of
processing one of these packets:
- `ActorInstantiate` (registers the Lua class binding)
- `SetActorProperty("/_init")` (the 3 init flag bytes)
- `SetEventStatus` (each condition write)

Garlemald has all 3 packet builders:
- `build_actor_instantiate` — `packets/send/actor.rs:63`
- `build_set_notice_event_condition` —
  `packets/send/actor_events.rs:78`
- `build_set_event_status` — `packets/send/actor.rs:377`

And the actor structs already carry the event condition lists:
`crate::actor::event_conditions::EventConditionList` at
`actor/mod.rs:91`, populated from `actor_class.event_conditions`.

## Proposed fix

Add a new step **between steps 4 and 5** that re-broadcasts the
FULL three-packet-group per-actor sequence for every actor in
the destination zone EXCEPT the warping player and the login
director (which step 4 already handles).

### Implementation sketch — REVISED for full 3-group fix

After the existing `send_zone_in_bundle` call in
`apply_do_zone_change_content`:

The minimal viable approach is to introduce a new helper that
emits the full pmeteor-equivalent sequence, since extending
`spawn_bundle_fanout` would risk regressing the normal mob-spawn
path (which currently works fine).

**New helper** (proposed location:
`map-server/src/runtime/dispatcher.rs`, alongside
`spawn_bundle_fanout`):

```rust
/// Emits the full pmeteor-equivalent per-actor packet sequence
/// for re-broadcast scenarios (post-warp DeleteAllActors recovery).
/// Mirrors pmeteor's `actor.GetSpawnPackets(player, 1)` +
/// `actor.GetInitPackets()` + `actor.GetSetEventStatusPackets()`.
///
/// Differs from `spawn_bundle_fanout` in three ways:
/// 1. Uses AddActor flag=8 (non-self spawn) instead of flag=0
/// 2. Emits ActorInstantiate + GetInitPackets + SetEventStatus
///    sequence to set up the actor's class binding + event
///    condition list on the client side.
/// 3. Targets a SPECIFIC session (the warping player), not
///    "all neighbours via spatial broadcast".
pub(crate) async fn re_spawn_actor_for_session(
    world: &WorldManager,
    registry: &ActorRegistry,
    target_session_id: SessionId,
    actor_id: u32,
) {
    let Some(handle) = registry.get(actor_id).await else { return };
    let Some(client) = world.client(target_session_id).await else { return };

    let (
        name, state, display_name_id,
        position, rotation,
        model_id, appearance_ids,
        event_conditions, class_path, class_params,
    ) = {
        let c = handle.character.read().await;
        (
            c.base.display_name().to_string(),
            c.base.current_main_state as u8,
            c.base.display_name_id,
            c.base.position(),
            c.base.rotation,
            c.chara.model_id,
            c.chara.appearance_ids,
            c.chara.event_conditions.clone(),
            c.chara.class_path.clone(),    // verify this field exists
            c.chara.class_params.clone(),  // verify this field exists
        )
    };

    // Group 1: GetSpawnPackets — 8 packets
    let mut packets: Vec<Vec<u8>> = vec![
        tx::actor::build_add_actor(actor_id, 8).to_bytes(),  // flag=8 for non-self
    ];
    packets.extend(
        tx::actor_events::build_event_condition_packets(actor_id, &event_conditions)
            .into_iter().map(|p| p.to_bytes())
    );
    packets.extend([
        tx::actor::build_set_actor_speed_default(actor_id).to_bytes(),
        tx::actor::build_set_actor_position(
            actor_id, -1, position.x, position.y, position.z, rotation, 1, false,
        ).to_bytes(),
        tx::actor::build_set_actor_name(actor_id, display_name_id, &name).to_bytes(),
        tx::actor::build_set_actor_state(actor_id, state, 0).to_bytes(),
        tx::actor::build_set_actor_is_zoning(actor_id, false).to_bytes(),
        tx::actor::build_actor_instantiate(
            actor_id, 0, 0x3040, &name, &class_path, &class_params,
        ).to_bytes(),
    ]);

    // Group 2: GetInitPackets — single SetActorProperty("/_init") with 3 flags
    packets.push(tx::actor::build_actor_property_init(actor_id).to_bytes());

    // Group 3: SetEventStatus per condition
    packets.extend(
        tx::actor::build_set_event_status_packets(actor_id, &event_conditions)
            .into_iter().map(|p| p.to_bytes())
    );

    for bytes in packets {
        client.send_bytes(bytes).await;
    }
}
```

**Caller site** in `apply_do_zone_change_content` (between steps
4 and 5):

```rust
// 4.5. Re-spawn area actors after the DeleteAllActors wipe.
//      Emits the FULL pmeteor-equivalent per-actor sequence
//      (Spawn + Init + EventStatus) so the client's actor
//      registry, Lua class bindings, and event-condition lists
//      are all re-established. Without this, the trailing
//      KickEvent and any subsequent RunEventFunction packets
//      silently drop because the target actor's +0x5c / +0x7d
//      flags are unset.
//
//      Mirrors pmeteor:
//        playerSession.ClearInstance();
//        player.SendInstanceUpdate(true);
//      from Map Server/WorldManager.cs:1004-1006.
let login_director_actor_id = self
    .world
    .session(session_id)
    .await
    .and_then(|s| s.login_director.as_ref().map(|d| d.actor_id));
let active_director_actor_id = self
    .world
    .session(session_id)
    .await
    .and_then(|s| s.active_content_script.as_ref().map(|d| d.director_actor_id));

let actors_to_respawn: Vec<u32> = if let Some(zone) = self.world.zone(parent_zone_id).await {
    let z = zone.read().await;
    z.core
        .iter()
        .filter(|a| a.actor_id != actor_id)                  // skip warping player
        .filter(|a| Some(a.actor_id) != login_director_actor_id)
        // Don't filter active_content_director here — it's a SEPARATE
        // entity that needs re-spawning if it's in the destination zone.
        // (TODO: confirm whether content directors live in the dest
        // zone's actor list or are tracked elsewhere.)
        .map(|a| a.actor_id)
        .collect()
} else {
    Vec::new()
};

for npc_actor_id in &actors_to_respawn {
    crate::runtime::dispatcher::re_spawn_actor_for_session(
        &self.world,
        &self.registry,
        session_id,
        *npc_actor_id,
    )
    .await;
}

tracing::info!(
    player = player_id,
    parent_zone = parent_zone_id,
    respawned = actors_to_respawn.len(),
    "DoZoneChangeContent: re-broadcast Spawn+Init+EventStatus for area actors after wipe",
);
```

(Note: `client_blocking_dir` in the earlier sketch was a
placeholder; this version uses the existing async
`world.session(...)` API.)

### Open implementation questions to verify before applying

- **`Character.chara.class_path` / `Character.chara.class_params`
  fields** — used in the sketch above. Need to verify these
  fields exist on `Character` (or wherever the Lua class
  binding info lives). If not, the caller of
  `re_spawn_actor_for_session` will need to supply these from a
  different source (e.g., the `actor_class` row that was used
  for the original spawn).
- **`build_event_condition_packets` / `build_set_event_status_packets`**
  — the sketch assumes batch helpers exist. If only per-
  condition builders exist, the helper needs to iterate the
  `EventConditionList` and call them one at a time.
- **`build_actor_property_init`** — exists in
  `world_manager.rs:61` (`build_director_init_packet` may be
  the analog). Need to confirm the actor variant exists for
  general (non-director) actors. If not, write a parallel
  helper that emits the same 3-byte-flag pattern (0xE14B0CA8,
  0x2138FD71, 0xFBFBCFB1).
- **Active content director (vs login director)** — the
  active director (e.g. `OpeningDirector` for the man0g0 quest)
  is in the `active_content_script` session field. Whether it
  lives in `dest_zone.core.iter()` or is tracked elsewhere
  needs confirmation. If it's in the zone's actor list, the
  re-spawn loop above WILL re-spawn it (good). If it's in a
  separate per-session director registry, we need an
  additional re-spawn call for it.

### Considerations + risks (revised for the 3-group helper)

1. **The new `re_spawn_actor_for_session` helper sends to a
   specific session, NOT spatially.** This is intentional — the
   warping player is the only client whose actor list was just
   wiped. Other players in the area (in non-instanced content)
   already have the actors on their client; spatially broadcasting
   would duplicate-send to them. Per-session targeting avoids that.

2. **Does the helper cover director actors?** Probably not, and
   we shouldn't ask it to. Directors are tracked separately
   (via `session.login_director` spec for login director, via
   `session.active_content_script.director_actor_id` for content
   directors). Both have their own spawn-packet sequences:
   - Login director: emitted by `send_zone_in_bundle` (already
     handled in step 4)
   - Content director: needs verification — see open question
     below
   The helper should filter out actor_ids matching either
   director.

3. **CONTENT directors vs LOGIN directors.** Content directors
   (the `OpeningDirector` / `man0g0` quest director) are spawned
   by `apply_create_content_area` (Phase A of SEQ_005). After
   the warp, the content director needs to be on the client
   too — but `send_zone_in_bundle` only handles the LOGIN
   director. Two options to investigate:
   - If `dest_zone.core.iter()` includes the content director's
     actor_id (because `apply_create_content_area` registered it
     in the area), the helper might handle it — but only if the
     registry returns a `Character` for it. Directors typically
     are NOT `Character`-backed in garlemald.
   - If neither path covers it, we need a director-specific
     re-spawn path (mirroring `send_zone_in_bundle`'s login-
     director handling but for the content director).

4. **Don't re-fire `onSpawn` per re-spawned actor.** Pmeteor's
   `UpdateInstance(true)` doesn't — it's a wire-level
   re-broadcast, not a logical re-spawn. The Lua `onSpawn` hooks
   already ran when the actors were FIRST spawned (via
   `apply_create_content_area` for content actors, or via
   zone-load for seed actors). Re-firing them on warp would
   double-fire side effects.

5. **Performance.** A content area might have ~5-30 actors.
   Each `re_spawn_actor_for_session` call emits ~10-15 packets
   (1 AddActor + N event-condition packets + 6 spawn-helper
   packets + 1 init packet + N event-status packets). So 50-450
   packets in a tight loop after the warp. Acceptable for a
   one-shot warp event but worth measuring if it causes
   noticeable lag. (Pmeteor has the same overhead; it's not a
   regression.)

6. **Idempotence on the client side.** If the spawn packets land
   for actors the client already knows about (e.g. from a prior
   partial state), the client should idempotently re-set the
   fields. Verify against pmeteor: their `UpdateInstance` skips
   actors already in the session's `actorInstanceList`, so they
   never double-spawn. We don't have an `actorInstanceList`-
   equivalent; the `force=true` path in pmeteor sends to all
   actors regardless. Likely safe but worth testing.

### Test plan

1. **Smoke test (the original blocker):** Run
   `fresh-start-gridania.sh`, progress through man0g0 to the
   second Yda conversation. The "Now Loading" hang should clear
   and the SEQ_005 combat tutorial should begin.
2. **Cross-zone warp regression:** Run any zone-change quest
   (e.g. Gridania → Black Shroud transition). Confirm normal
   warps still work and don't double-spawn the player.
3. **Multi-player content area:** If garlemald supports multi-PC
   content areas (instances), confirm second player's view isn't
   corrupted by the re-broadcasts.
4. **Packet capture diff:** Compare the post-fix garlemald
   packet log against the pmeteor packet log for the same warp
   to confirm the wire-level shapes match.

### Cross-references

- Existing per-actor spawn helper:
  `crate::runtime::dispatcher::spawn_bundle_fanout` at
  `map-server/src/runtime/dispatcher.rs:1136` — emits ONLY
  group 1 (AddActor + position + appearance + state etc., no
  ActorInstantiate, no init properties, no event status). The
  proposed `re_spawn_actor_for_session` helper is a strict
  superset adding groups 2 and 3.
- Existing zone API: `Zone.core.iter()` returns
  `Iterator<Item=&StoredActor>` at
  `map-server/src/zone/area.rs:457`.
- Existing director-respawn: `send_zone_in_bundle` already
  prepends login director's spawn packets at
  `map-server/src/world_manager.rs:1231-1318`.
- Existing director-init helper: `build_director_init_packet`
  at `map-server/src/world_manager.rs:61` — model for the
  general `build_actor_property_init` we may need to add.
- Pmeteor reference: `Map Server/WorldManager.cs:1004-1006`
  (`playerSession.ClearInstance()` + `player.SendInstanceUpdate(true)`
  in `DoZoneChangeContent`); `Map Server/DataObjects/Session.cs:112-170`
  (`UpdateInstance` body); `Map Server/Actors/Actor.cs:257-360`
  (`GetSpawnPackets` / `GetInitPackets` / `GetSetEventStatusPackets`).
- Meteor-decomp reference: `event_kick_receiver_decomp.md`
  (`+0x5c` gate); `event_run_event_function_receiver_decomp.md`
  (`+0x7d` gate); `reference_ffxiv_1x_actor_event_flags.md`
  (memory entry summarizing both flags).

### Failure-mode caveat

The fix addresses two of three plausible hang causes:
- **(A)** Cinematic-start KickEvent silently drops because target
  director's `+0x5c` is unset → fix re-spawns directors with
  full sequence → addressed
- **(B)** Zone-in bundle itself is missing something that signals
  load-complete to the client → **NOT addressed by this fix**
- **(C)** Cinematic starts but RunEventFunction targeting a
  BattleNpc silently drops → fix re-spawns BattleNpcs with
  EventStatus packets → addressed

If the hang persists after applying this fix, (B) is the
remaining suspect — investigate by capturing the post-fix
packet trace and diffing against pmeteor's same-scenario trace.
