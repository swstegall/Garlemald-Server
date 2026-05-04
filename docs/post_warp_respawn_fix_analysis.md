# Man0g0 SEQ_005 hang — diagnosis, smoke-test journey, and remaining gap

> Drafted 2026-05-04 from meteor-decomp's Phase 7 cinematic-receiver
> findings. End-to-end pre-work + smoke-test debugging for the man0g0
> SimpleContent cinematic hang at "Now Loading" after talking to Yda
> the second time.
>
> **THREE iterations** of the analysis as smoke testing surfaced
> deeper layers:
>   1. First analysis (preserved at the bottom as historical
>      context): hypothesized a missing post-warp NPC respawn.
>   2. Second iteration: codebase audit invalidated #1 —
>      `send_zone_in_bundle` already handles area NPCs. Bug is
>      really in CONTENT DIRECTOR spawn (never emitted on client).
>   3. Third iteration (THIS doc): smoke testing of the #2 fix
>      surfaced 5 layers of bugs (all fixed) plus a deeper
>      remaining gap — the director's main coroutine doesn't
>      progress post-warp. See "Smoke-test journey" + "Remaining
>      gap" sections below.

## TL;DR for the next session

The original "missing CONTENT DIRECTOR spawn" hypothesis was
**partially right** — the spawn WAS missing, but fixing it
unblocks only one of multiple layers. The remaining blocker is
the director-coroutine driver not waking post-warp, which is the
known SEQ_005 unblock-plan gap (memory entry
`project_garlemald_man0g0_seq005_complete.md`).

Today's session landed:
- 5 layered bug fixes (all committed: see "Smoke-test journey")
- Wire-level groundwork is now solid: client correctly receives
  director spawn + SynchGroupWorkValues `/_init` reply
- No more crashes; client cleanly hangs on Now Loading instead

What's left:
- Director main-coroutine driver needs to resume after warp
  completion + drive its `delegateEvent` calls into wire packets
  via the event-outbox dispatcher (not the login-scoped
  dispatcher which silently drops them).

## Smoke-test journey (chronological — 5 fixes today)

Each fix exposed the next layer. Useful for the next person
picking this up to know what's already settled vs what's left.

| # | Symptom | Root cause | Fix | Commit |
|---|---|---|---|---|
| 1 | Wine crash on first cutscene | `apply_create_content_area` emitted director spawn BEFORE warp completed → double-spawn (since SetLoginDirector fires shortly after, routing same actor through the existing login-director path) | Reverted Step-A emission | `7cac01e` |
| 2 | Wine NULL+0x5C page-fault crash post-warp | Director's `class_name` was path-style (`Quest/QuestDirectorMan0g001`) → embedded slash in actor-name format → client actor-table lookup failed → ActorInstantiate couldn't construct the Lua object → downstream code wrote to NULL+0x5C | Strip path to leaf in `build_director_spawn_subpackets`; mirror pmeteor `Director.cs` `className.Substring(LastIndexOf("/")+1)` | `863ce73` |
| 3 | (Original gap) CONTENT director not spawned on client at all | `apply_create_content_area` only fired the script's `onCreate`; never emitted director spawn packets. `send_zone_in_bundle`'s respawn loop filters out non-Character actors (directors slip past). | Refactored director-spawn into reusable `build_director_spawn_subpackets` helper + added content-director block to `send_zone_in_bundle` reading from `session.active_content_script` | `eb7c573` |
| 4 | Now Loading hang post-warp despite director construct succeeding | Client sent `0x0133 GroupCreated` for director's `/_init`, expecting `0x017A SynchGroupWorkValues` reply with the content-group's director property + property[0] byte. Garlemald's handler was a no-op. | Wired `build_synch_group_work_values_content_init` builder + handler that responds when `event_name == "/_init"` | `adc3244` |
| 5 | Regression — game crashed before first cutscene at character creation | Previous fix's reply was too aggressive: OpeningDirector path also sends 0x0133 for player-work group (high bit set, `group_id=0x8000000000000001`). Treating its low-u32 as a director actor id sent malformed reply → Wine crash. | Filter the reply to high-u32 == 0 only (content director ids; player-work groups have high bit set, mob groups have `0x2680...` prefix) | `175f53d` |

After all 5 fixes, the smoke-test state is:
- ✓ Character creation cinematic plays (no crash)
- ✓ Quest progression up to second-Yda talk works
- ✓ Warp into man0g0 SEQ_005 area completes cleanly
- ✓ Director spawn lands; `0x0133` reply emitted; client receives it
- ✗ "Now Loading" persists — director's cinematic body never runs

## Remaining gap (the deeper subsystem)

After the SynchGroupWorkValues reply lands, the server-side log
shows ONLY pings + the client doesn't send any further packets.
Crucially, **no `RunEventFunction` commands fire on the server
side after the warp** — meaning the director's main coroutine
(spawned pre-warp via `StartDirectorMain`) isn't being driven
forward to execute the cinematic body.

Server-side timeline at the hang:

```
T+0  CreateContentArea + onCreate runs
     → SpawnBattleNpcById x5 (Yda + Papalymo + 3 wolves)
     → PartyAddMember + DirectorAddMember
     → StartDirectorMain (director main coroutine spawned)
     → KickEvent captured for after-warp emission
T+0  SetLoginDirector + login director spawn packets prepended
T+0  KickEventPacket appended
T+0  zone-in bundle dispatched
T+0  DoZoneChangeContent applied
T+0  RX 0x0133 GroupCreated (event=/_init)
T+0  → emitted SynchGroupWorkValues /_init reply
T+1+ ONLY pong packets — director coroutine never resumes
```

The director's main coroutine needs to:
1. Wake after warp completion (signal: zone-in done, or
   client's `/_init` ack received)
2. Pump its `coroutine.yield("_WAIT_EVENT", player)` returns
3. Drive `delegateEvent` Lua calls into wire `RunEventFunction`
   packets via the **event-outbox dispatcher**, not the
   login-scoped dispatcher (which currently silently drops them
   — see `apply_login_lua_command`'s "unhandled" branch logged
   as `login lua cmd (unhandled) other=RunEventFunction { ... }`)

There's a parallel, working path in
`processor.rs::fire_quest_event_hook` that translates event-
flavoured commands via `EventOutbox` → `dispatch_event_event`
(comment at `processor.rs:5912-5934` explains the pattern).
The director coroutine driver needs to use the same translation
mechanism rather than going through `apply_login_lua_command`.

This is **the same blocker** memory entry
`project_garlemald_man0g0_seq005_complete.md` flagged: "only
blocker is SEQ_005 combat tutorial behind DoZoneChangeContent
stub." Today's work cleared the wire-level prerequisites
(spawns, replies, no crashes); the remaining work is the
coroutine-driver subsystem itself.

## Recommended next-session approach

1. **Investigate `StartDirectorMain` coroutine lifecycle.** Where
   does the spawned coroutine sleep? What's supposed to wake it?
2. **Trace the man0g0 SEQ_005 director script** — what Lua hooks
   does the script body actually call? Are they `noticeEvent`
   handlers that wait for the kick → `RunEventFunction` chain?
3. **Wire the coroutine-resume path post-warp.** Likely need a
   new ticker step or a hook in `apply_do_zone_change_content`'s
   onZoneIn callback that resumes the director coroutine and
   drives its commands through `EventOutbox` instead of the
   login-scoped dispatcher.

Estimated scope: multi-hour focused session. The
`fire_quest_event_hook` pattern can be the model for the
coroutine driver's command-dispatch translation.

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
