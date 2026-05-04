# Man0g0 SEQ_005 hang — corrected diagnosis + proposed fix

> Drafted 2026-05-04 from meteor-decomp's Phase 7 cinematic-receiver
> findings. Pre-work for the man0g0 SimpleContent cinematic hang at
> "Now Loading" after talking to Yda the second time.
>
> **Two iterations.** The first analysis (preserved at the bottom
> as historical context) hypothesized a missing post-warp NPC
> respawn. Codebase audit on 2026-05-04 invalidated that hypothesis
> — `send_zone_in_bundle` already iterates and respawns area NPCs
> within 50 units via `push_npc_spawn`, which already emits the
> full pmeteor-equivalent 3-group sequence. The actual bug is
> different + smaller in scope.

## The real bug

The SEQ_005 cinematic targets a CONTENT DIRECTOR (created by
`apply_create_content_area` when the SEQ_005 area starts). The
content director is **never spawned on the client side** —
neither at content-area creation time nor at post-warp respawn
time. So when KickEvent / RunEventFunction targeting this
director arrives, the client looks it up, finds nothing, and
the kick silently drops per the meteor-decomp slot 2 gate
(`actor[+0x5c] != 0`).

Smoking gun: the comment at
`map-server/src/director/dispatcher.rs:23-27` explicitly says:

> "Full director-actor-spawn packets (ActorInstantiate + Init)
> need the director's Lua class path + params — those come from
> the Phase 4 event dispatcher when the script runs. Here we hand
> off the member-facing side effects..."

i.e., **the director spawn packets were always intended to be
emitted somewhere else and never landed**. Today the only
director-spawn path is the LOGIN director block in
`send_zone_in_bundle` (which fires for the OpeningDirector at
character-create time). Content directors created later via
`apply_create_content_area` have no equivalent emission path.

## What's already working (don't touch)

| Element | Status | Where |
|---|---|---|
| Area NPCs (BattleNpcs, MapObjs) post-warp respawn | ✅ Working | `world_manager.rs:1773` (loop in `send_zone_in_bundle`) |
| `push_npc_spawn` emits the full pmeteor 3-group sequence | ✅ Working | `world_manager.rs:259-490` |
| LOGIN director (e.g., OpeningDirector) initial spawn | ✅ Working | `world_manager.rs:1231-1318` (in `send_zone_in_bundle`) |
| LOGIN director post-warp respawn | ✅ Working | Same block (re-fires every time) |

## What's broken

| Element | Status | Why |
|---|---|---|
| **CONTENT director initial spawn on client** | ❌ Missing | `apply_create_content_area` (`processor.rs:1708`) only fires the content script's `onCreate`; doesn't emit director spawn packets to the client |
| **CONTENT director post-warp respawn** | ❌ Missing | Even if Step A worked, `send_zone_in_bundle`'s respawn loop filters out non-Character actors (`if !matches!(kind, Npc/BattleNpc/Ally) continue`), so directors slip past |

## What actually happens at the second-Yda talk

1. Player talks to Yda 2nd time → quest fires `CreateContentArea` Lua
2. `apply_create_content_area` runs:
   - Logs the creation
   - Fires content script's `onCreate` → spawns BattleNpcs (Yda, Papalymo, 3 wolves) via `SpawnBattleNpcById`
   - Spawns are session-broadcast normally (✓ shows on client)
   - **Content director registered server-side but NO spawn packets sent**
3. `apply_do_zone_change_content` runs:
   - DeleteAllActors (wipe)
   - 0xE2(0x10) marker
   - `send_zone_in_bundle`:
     - Builds master bundle (player self-spawn + login director + ...)
     - Sends master bundle session-targeted
     - Iterates `zone.core.actors_around(player, 50)` filtered to NPC/BattleNpc/Ally
     - Calls `push_npc_spawn` for each → re-spawns the BattleNpcs ✓
     - **Content director skipped — not in `actors_around` AND/OR not in the kind filter**
   - Fires `onZoneIn` content-script hook
4. Cinematic tries to fire via KickEvent / RunEventFunction targeting the content director:
   - Client receives KickEvent
   - `KickClientOrderEventReceiver::Receive` (slot 2) calls `ActorRegistry_lookup_actor(director_id)`
   - Lookup returns NULL because director was never spawned
   - **KickEvent silently drops; cinematic never starts; "Now Loading" forever**

## Proposed fix (REVISED — smaller + more focused)

Two changes, ordered by leverage:

### Change A — Emit content director spawn packets at content-area creation

In `apply_create_content_area` (`processor.rs:1708`), after the
`onCreate` Lua dispatch completes, build + send the director
spawn-packet sequence to the calling player's session.

**Reuse the existing login-director spawn template** at
`world_manager.rs:1231-1318` — same set of packets:

```
build_add_actor(director_id, 0)
+ build_set_notice_event_condition_raw × 3 (noticeEvent / noticeRequest / reqForChild)
+ build_set_actor_speed_default
+ build_set_actor_position
+ build_set_actor_name
+ build_set_actor_state
+ build_set_actor_is_zoning(false)
+ build_actor_instantiate(...)
+ build_director_init_packet
```

**Refactor opportunity**: extract this 10-packet template into a
new helper `build_director_spawn_subpackets(spec: &DirectorSpec)
-> Vec<SubPacket>` in `world_manager.rs`. Both the existing
login-director block AND the new content-director path call it.
Reduces drift risk (one place to maintain the canonical director
spawn shape).

### Change B — Re-spawn content director in `send_zone_in_bundle`

Add a parallel block alongside the existing login-director-spawn
block. Read `session.active_content_script.director_actor_id` +
`active_content_script.area_class_path` (both already on the
session), build a `DirectorSpec` for the content director, call
`build_director_spawn_subpackets`, push the result into
`subpackets`.

This block should fire AFTER the login-director block so the
ordering matches what the cinematic state machine expects (login
director established first, then content director layered on
top).

### Implementation sketch

```rust
// In world_manager.rs, alongside build_director_init_packet:
fn build_director_spawn_subpackets(
    spec: &DirectorSpec,
    zone_short: &str,
    zone_actor_id: u32,
) -> Vec<common::subpacket::SubPacket> {
    let class_lower = lowercase_first(&spec.class_name);
    let max_class_len = 20usize.saturating_sub(zone_short.len());
    let class_lower = truncate(class_lower, max_class_len);
    let director_actor_name = format!(
        "{class_lower}_{zone_short}_0@{zone_actor_id:03X}00",
    );
    let director_bind_params = vec![
        common::luaparam::LuaParam::String(spec.class_path.clone()),
        common::luaparam::LuaParam::False,
        common::luaparam::LuaParam::False,
        common::luaparam::LuaParam::False,
        common::luaparam::LuaParam::False,
        common::luaparam::LuaParam::False,
    ];
    vec![
        tx::actor::build_add_actor(spec.actor_id, 0),
        tx::actor_events::build_set_notice_event_condition_raw(
            spec.actor_id, 0x0E, 0x00, "noticeEvent",
        ),
        tx::actor_events::build_set_notice_event_condition_raw(
            spec.actor_id, 0x00, 0x01, "noticeRequest",
        ),
        tx::actor_events::build_set_notice_event_condition_raw(
            spec.actor_id, 0x00, 0x01, "reqForChild",
        ),
        tx::actor::build_set_actor_speed_default(spec.actor_id),
        tx::actor::build_set_actor_position(
            spec.actor_id, spec.actor_id as i32,
            0.0, 0.0, 0.0, 0.0, 0x0, false,
        ),
        tx::actor::build_set_actor_name(
            spec.actor_id, 0, &director_actor_name,
        ),
        tx::actor::build_set_actor_state(spec.actor_id, 0, 0),
        tx::actor::build_set_actor_is_zoning(spec.actor_id, false),
        tx::actor::build_actor_instantiate(
            spec.actor_id, 0, 0x3040,
            &director_actor_name, &spec.class_name,
            &director_bind_params,
        ),
        build_director_init_packet(spec.actor_id),
    ]
}
```

(Refactoring the existing login-director block to call this is a
follow-up — the existing block can stay inline initially, the new
content-director block calls the helper.)

Caller site for **Change A** (in `apply_create_content_area`,
after the `onCreate` dispatch):

```rust
// After the LuaCommand dispatch completes:
let Some(handle) = self.registry.get(player_id).await else {
    return;
};
let session_id = handle.session_id;
let Some(client) = self.world.client(session_id).await else {
    return;
};
let director_class_name = director_name.clone();
let director_class_path = format!("/Director/{}", director_name);
let zone_arc = match self.world.zone(parent_zone_id).await {
    Some(z) => z,
    None => return,
};
let zone_name = {
    let z = zone_arc.read().await;
    z.core.zone_name.clone()
};
let zone_short = shorten_zone_name(&zone_name);
let zone_actor_id = parent_zone_id;
let spec = DirectorSpec {
    actor_id: director_actor_id,
    zone_actor_id,
    class_path: director_class_path,
    class_name: director_class_name,
};
let subpackets = build_director_spawn_subpackets(&spec, &zone_short, zone_actor_id);
for mut sub in subpackets {
    sub.set_target_id(session_id);
    client.send_bytes(sub.to_bytes()).await;
}
tracing::info!(
    director = director_actor_id,
    class = %director_name,
    "CreateContentArea: emitted director spawn packets to client",
);
```

Caller site for **Change B** (in `send_zone_in_bundle`, after the
existing login-director block at line ~1318):

```rust
// Mirror the login-director spawn block for the active content
// director. Without this, post-warp KickEvent / RunEventFunction
// targeting the content director silently drops on the client
// (the director isn't in the post-DeleteAllActors actor list).
if let Some(active) = session.active_content_script.as_ref() {
    let content_spec = DirectorSpec {
        actor_id: active.director_actor_id,
        zone_actor_id: parent_zone_id,
        class_path: active.area_class_path.clone(),
        class_name: active.director_name.clone(),
    };
    subpackets.extend(build_director_spawn_subpackets(
        &content_spec, &zone_short, parent_zone_id,
    ));
    tracing::info!(
        director = content_spec.actor_id,
        "send_zone_in_bundle: prepended content director spawn packets"
    );
}
```

### Open implementation questions

1. **Does `apply_create_content_area` already have all the data?**
   - `director_name` ✓ (passed as parameter)
   - `director_actor_id` ✓ (passed as parameter)
   - `area_class_path` ✓ (passed as parameter)
   - `parent_zone_id` ✓ (passed as parameter)
   - Player session_id — needs lookup via `self.registry.get(player_id)` ✓
   - `zone_name` for the actor-name format — needs lookup via `self.world.zone(parent_zone_id)` ✓

2. **Is `DirectorSpec` already defined?**
   - `LoginDirectorSpec` exists in `data.rs` for the login path.
   - Either reuse it (rename to `DirectorSpec`) or add a parallel
     `ContentDirectorSpec` if the field shapes diverge.

3. **`area_class_path` vs `director.class_path`** — need to verify
   the active_content_script's stored `area_class_path` is the
   correct value to pass to `build_actor_instantiate` for the
   director. It might need to be `/Director/<DirectorName>`
   instead of the area class path. Check what pmeteor sends in
   its director-spawn packet.

### Test plan

1. **Smoke test (the original blocker):** Run
   `fresh-start-gridania.sh`, progress through man0g0 to the
   second Yda conversation. The "Now Loading" hang should clear
   and the SEQ_005 combat tutorial should begin.
2. **Cross-zone warp regression:** Confirm normal zone-change
   quests still work without double-spawning anything.
3. **CreateContentArea-then-warp regression:** Verify the
   director isn't double-spawned (once at CreateContentArea,
   once at post-warp send_zone_in_bundle). The client should
   handle the duplicate AddActor idempotently — but worth
   confirming.
4. **Multi-content-area regression:** If multiple content areas
   can be created in sequence, confirm only the active one's
   director spawn is re-fired post-warp (`session.
   active_content_script` is single-valued so this should be
   automatic).
5. **Packet capture diff:** Compare post-fix garlemald packet
   log against pmeteor's same scenario; should now match for
   director-spawn packet shape.

### Cross-references

- **Existing login-director spawn template:**
  `map-server/src/world_manager.rs:1231-1318` — the inline
  block this fix mirrors for the content director.
- **Existing director init helper:**
  `map-server/src/world_manager.rs:61` (`build_director_init_packet`).
- **Existing per-NPC spawn helper:**
  `map-server/src/world_manager.rs:259` (`push_npc_spawn`) —
  emits the full pmeteor 3-group sequence; already used for
  area NPCs in `send_zone_in_bundle`'s respawn loop.
- **Where the bug is:**
  `map-server/src/processor.rs:1708` (`apply_create_content_area`)
  doesn't emit director spawn packets after `onCreate`.
- **Smoking gun comment:**
  `map-server/src/director/dispatcher.rs:23-27` — explicitly
  marks director-spawn-packet emission as deferred.
- **Pmeteor reference:** `Map Server/Actors/Director/Director.cs`
  — pmeteor's `Director.GetSpawnPackets` is the canonical
  director-spawn template that pmeteor emits at content area
  creation + during `Session.UpdateInstance(true)`.
- **Meteor-decomp reference:**
  `event_kick_receiver_decomp.md` (`+0x5c` gate explanation).

---

## (Historical) First-iteration analysis — superseded 2026-05-04

> Original hypothesis: missing post-warp NPC respawn. Invalidated
> by codebase audit which showed `send_zone_in_bundle` already
> handles area NPC respawn correctly via `push_npc_spawn` (which
> already emits all 3 packet groups: Spawn + Init + EventStatus).
> Kept here for cross-reference; the corrected diagnosis above is
> what should be implemented.

The original analysis proposed a new helper
`re_spawn_actor_for_session` that would emit the full pmeteor
3-group sequence per actor. Investigation found that:

- `push_npc_spawn` (`world_manager.rs:259-490`) already emits all
  3 groups (Spawn + Init via `build_npc_property_init` + EventStatus
  via `build_actor_event_status_packets`).
- `send_zone_in_bundle` already calls it for area NPCs within 50
  units of the warping player (loop at line 1773).

So the proposed fix would have been redundant. The actual gap is
narrower: the **content director** specifically is never spawned
on the client, and that's what the man0g0 SEQ_005 cinematic
targets after the warp.
