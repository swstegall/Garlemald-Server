require ("global")
require ("modifiers")
-- onUpdate drives ally/mob engagement through allyGlobal.EngageTarget;
-- without this require the global is nil and every tick errors
-- (swallowed by the ticker drain). Same shape as SimpleContent30002.
require ("ally")
-- SEQ_* constants + the escort TEXT_*/SAY_* constants (proven loadable
-- outside the quest runtime — QuestDirectorMan0l101 requires it too).
require ("quests/man/man0l1")

-- Man0l1 "Treasures of the Main" SEQ_050 — the Zephyr Gate → Oschon's
-- Torch escort duty (Garlemald-Server #46). Net-new content: upstream
-- pmeteor shipped the trigger arm commented out ("DO ESCORT DUTY HERE
-- ... For now just skip the sequence"), so there is no upstream
-- reference implementation — this is built on the #28 tutorial-fight
-- machinery (SpawnBattleNpcById onCreate spawning, the 500 ms onUpdate
-- driver, allyGlobal engagement, sendSignal → director coroutine).
--
-- Runtime shape (retail spec from OCR'd playthroughs + Mirke
-- transcripts): the duty runs as a content instance on ZONE 128 (retail
-- geography — see seed/070; the zone-129 Skull Valley pin from seed/066
-- is kept for rollback). Sisipu LEADS the walk due south from the gate
-- to the lighthouse approach along the ROUTE waypoints, barking her
-- say-283 guidance every ~20 s; the eight seeded ankle biters
-- (chigoes) sit ONE PER AMBUSH POINT at even ninths of the recorded
-- arc (screenshots show a single biter per location) and each pulls
-- alone as the party reaches its stretch. Arrival (both Sisipu and
-- the player inside the final-waypoint radius, road clear) fires
-- sendSignal("escortComplete") → the
-- director plays the processEvent605 arrival echo and warps to the
-- lighthouse camp. Failure (Sisipu death OR the 30-minute limit) tears
-- the content down, rolls the quest back to SEQ_048 (the ZEPHYR_TRIGGER
-- re-arms for retry) and ejects the player to the gate.

-- Escort pacing / geometry. The onUpdate driver ticks every 500 ms
-- (runtime/ticker.rs CONTENT_UPDATE_PERIOD_MS), so ticks = seconds * 2.
WALK_STEP = 1.6;             -- units per 500 ms tick (escort walking pace)
RUN_STEP = 3.4;              -- units per 500 ms tick (run pace — 7e walk was too slow)
HOLD_RADIUS = 28.0;          -- live mob within this of Sisipu/player → hold the walk
-- Render-before-fight invariant: ENGAGE_RADIUS (18) + PLAYER_LEASH (15)
-- stays below the server's INSTANCE_STREAM_RADIUS (50, world_manager.rs)
-- — as does the controller's MAX_DETECT_DISTANCE (20) — so every mob is
-- AddActor'd to the client before it can possibly pull or reach the
-- party. Widen these past 50 and mobs fight invisibly again (#46).
ENGAGE_RADIUS = 18.0;        -- mob pulls onto the escort party inside this
PLAYER_LEASH = 15.0;         -- Sisipu > this from the player → close the gap directly
WAYPOINT_RADIUS = 4.0;       -- close enough to the lead point → stop stepping
ARRIVAL_RADIUS = 20.0;       -- player inside this of the lighthouse goal → arrival
LEAD_DISTANCE = 5.0;         -- how far ahead of the player Sisipu paces while walking
MOVE_EPSILON = 0.3;          -- player displacement/tick above this = "walking"
TICKS_PER_SECOND = 2;
ESCORT_LIMIT_MINUTES = 30;   -- retail: "There are 30 minutes remaining."
ESCORT_LIMIT_TICKS = ESCORT_LIMIT_MINUTES * 60 * TICKS_PER_SECOND;
REMIND_20MIN_TICKS = 10 * 60 * TICKS_PER_SECOND;    -- 20 minutes remaining
REMIND_10MIN_TICKS = 20 * 60 * TICKS_PER_SECOND;    -- 10 minutes remaining
REMIND_5MIN_TICKS  = 25 * 60 * TICKS_PER_SECOND;    -- 5 minutes remaining
BARK_INTERVAL_TICKS = 40;                           -- guidance bark every ~20 s
ESCORT_DUTY_MUSIC = 37;      -- "Daring Dalliances" — 1.x quest-duty theme (wiki Music table id 37)
-- Wave-outcome bark thresholds: Sisipu's HP fraction at wave clear
-- picks the conditional variant (see the outcome beat in onUpdate).
HURT_HP_RATIO = 0.5;         -- at/below → "Ow ow ow..." (took a beating)
CLOSE_HP_RATIO = 0.85;       -- at/below → "Phew... much closer than I'd have liked"
-- Minimap duty halo: an invisible ContentPrivateAreaRange object
-- (seed/073 fills class 1290003's classPath + "exit"/"caution" push
-- circles). The client's DepictionJudge attaches a native range map
-- marker to any non-talkable actor whose class script defines
-- getMapMarkerRange() (depictionjudge.lua:773-791), radii resolved
-- from the wire-sent named push circles — so the halo rides this
-- object, and MoveTo'ing it along with Sisipu moves the quest area
-- ring on the minimap.
ESCORT_RING_CLASS = 1290003;
RING_MOVE_TICKS = 1;         -- re-home the halo EVERY 500 ms frame (one 0x00CF on one invisible marker — cheap; 4 lagged ~13.6u at run pace). #199

-- The engaged/defeated rows 30120/30121 ("<displayName> is engaged.")
-- take their subject from a type-6 actor-reference param: pass the mob's
-- handle, not its integer actor id (blank) or a DispId sender (renders a
-- "Ankle Biter:" chat prefix and still a blank subject). (#202)

-- Sisipu's path: the PLAYER'S OWN recorded footsteps (round 7f, extended
-- to the lighthouse in round 8). Decoded from the player's full
-- 2026-07-04 01:09-01:17 walk's inbound 0x00CA position packets (offset
-- verified against the warp anchor -63.25/33.16/164.51) and downsampled
-- to ~20-unit breadcrumbs: every point is REAL walked ground with true
-- terrain Y, so tracing them can never leave walkable land or float (the
-- navmesh is still a stub). The recording now reaches Oschon's Torch
-- itself — a 1720-unit arc ending at the lighthouse camp approach
-- (136.44, 60.70, 1324.01); one recorded backtrack step just before the
-- arrival was pruned so the tail runs smoothly. The player-relative
-- follow past the last breadcrumb is now only a safety fallback.
-- ARRIVAL_GOAL is pmeteor's SEQ_055 camp point.
TRAIL = {
	{ x = -63.25, y = 33.16, z = 164.51 },
	{ x = -45.02, y = 36.92, z = 155.90 },
	{ x = -26.65, y = 41.26, z = 147.70 },
	{ x = -6.55, y = 45.29, z = 140.92 },
	{ x = 14.93, y = 44.31, z = 140.65 },
	{ x = 36.27, y = 43.91, z = 139.11 },
	{ x = 55.80, y = 46.38, z = 134.14 },
	{ x = 76.48, y = 46.37, z = 129.19 },
	{ x = 97.21, y = 44.13, z = 131.30 },
	{ x = 110.60, y = 47.14, z = 147.63 },
	{ x = 130.32, y = 46.20, z = 155.32 },
	{ x = 149.47, y = 47.23, z = 164.72 },
	{ x = 159.39, y = 49.46, z = 182.96 },
	{ x = 161.77, y = 49.34, z = 203.54 },
	{ x = 154.03, y = 47.11, z = 222.07 },
	{ x = 145.69, y = 44.96, z = 241.43 },
	{ x = 139.74, y = 45.40, z = 261.82 },
	{ x = 133.33, y = 44.95, z = 281.98 },
	{ x = 123.39, y = 45.42, z = 300.94 },
	{ x = 110.17, y = 45.22, z = 317.74 },
	{ x = 98.67, y = 42.78, z = 335.21 },
	{ x = 87.31, y = 40.79, z = 353.31 },
	{ x = 80.17, y = 44.02, z = 373.29 },
	{ x = 73.08, y = 45.07, z = 393.45 },
	{ x = 70.13, y = 43.66, z = 414.70 },
	{ x = 70.11, y = 41.50, z = 435.66 },
	{ x = 69.88, y = 41.37, z = 456.91 },
	{ x = 70.77, y = 43.35, z = 477.76 },
	{ x = 87.71, y = 41.27, z = 489.08 },
	{ x = 108.77, y = 42.33, z = 492.30 },
	{ x = 124.48, y = 46.35, z = 506.46 },
	{ x = 131.73, y = 45.07, z = 526.60 },
	{ x = 139.03, y = 47.06, z = 546.87 },
	{ x = 139.60, y = 53.36, z = 567.82 },
	{ x = 134.90, y = 53.17, z = 588.17 },
	{ x = 134.03, y = 51.99, z = 608.77 },
	{ x = 131.58, y = 54.46, z = 629.89 },
	{ x = 127.75, y = 54.47, z = 650.68 },
	{ x = 123.68, y = 52.42, z = 671.72 },
	{ x = 121.86, y = 55.21, z = 692.62 },
	{ x = 118.85, y = 57.59, z = 713.07 },
	{ x = 106.80, y = 63.79, z = 729.79 },
	{ x = 86.30, y = 64.42, z = 730.03 },
	{ x = 72.22, y = 60.52, z = 745.81 },
	{ x = 58.58, y = 56.03, z = 761.97 },
	{ x = 53.61, y = 59.13, z = 782.49 },
	{ x = 48.21, y = 63.05, z = 802.34 },
	{ x = 27.39, y = 64.30, z = 805.55 },
	{ x = 7.02, y = 59.63, z = 810.46 },
	{ x = 2.98, y = 54.50, z = 831.35 },
	{ x = 3.93, y = 53.68, z = 852.32 },
	{ x = -1.11, y = 53.17, z = 873.06 },
	{ x = -6.17, y = 54.44, z = 893.87 },
	{ x = -14.89, y = 50.45, z = 912.89 },
	{ x = -22.95, y = 47.63, z = 932.25 },
	{ x = -18.26, y = 45.09, z = 951.77 },
	{ x = -6.85, y = 45.96, z = 969.73 },
	{ x = 6.68, y = 44.60, z = 985.20 },
	{ x = 19.95, y = 45.07, z = 1002.12 },
	{ x = 33.13, y = 44.08, z = 1018.93 },
	{ x = 46.17, y = 43.14, z = 1035.55 },
	{ x = 57.56, y = 44.11, z = 1053.79 },
	{ x = 63.09, y = 46.25, z = 1073.98 },
	{ x = 66.75, y = 46.46, z = 1095.15 },
	{ x = 72.18, y = 44.73, z = 1116.02 },
	{ x = 80.52, y = 44.54, z = 1135.35 },
	{ x = 95.82, y = 44.01, z = 1150.23 },
	{ x = 111.13, y = 44.66, z = 1165.12 },
	{ x = 123.30, y = 44.07, z = 1181.26 },
	{ x = 124.53, y = 46.16, z = 1202.12 },
	{ x = 114.03, y = 46.12, z = 1220.71 },
	{ x = 107.51, y = 50.24, z = 1241.09 },
	{ x = 113.16, y = 57.56, z = 1261.33 },
	{ x = 123.01, y = 62.47, z = 1278.80 },
	{ x = 127.12, y = 61.61, z = 1298.65 },
	{ x = 137.44, y = 60.33, z = 1322.00 },
	{ x = 137.74, y = 60.41, z = 1323.04 },
	{ x = 136.44, y = 60.70, z = 1324.01 },
};
ARRIVAL_GOAL = { x = 137.44, y = 60.33, z = 1322.0 };

-- Sisipu's bark slots — man0l1 QUEST-sheet rows decoded straight from
-- the client's text sheets (data/03/A2/04/4A.DAT EN + 4B enable; rows
-- [u16 len][u8][bytes XOR 0x73], ids from the enable run file — the
-- SeventhUmbral SheetData.cpp:ReadRow format). Delivery = emitBark →
-- owner:SendGameMessage(quest, sayId, 0x20) with the Man0l1 quest
-- actor as text owner. (Say 119 is her SEQ_055 camp talk and does NOT
-- belong here; row 284 — the "repellant draws beasts" ambush line —
-- and the 291/292 arrival follow-ups are unwired spares.)
BARKS = {
	dutyStart  = 105,   -- "Excited yet? I know I am. Now let us be off!..."
	guidance   = SAY_ESCORT_GUIDANCE,   -- 283 "Oschon's Torch is due south..."
	waveClean  = 286,   -- "Good show! Perhaps you are not as useless..."
	waveClose  = 287,   -- "Phew... That was much closer than I'd have liked..."
	waveThanks = 288,   -- "Thank the Navigator! Oh, and thank you, as well."
	sisipuHurt = 289,   -- "Ow ow ow... That's going to leave a nasty bruise..."
	dawdle     = 285,   -- "Now put away your toys and come along, <name>..."
	arrival    = 290,   -- "We've finally arrived...and in one piece!"
};

-- Per-player escort state, keyed by actor id: the VM is process-cached
-- per script path, so a plain scalar would interleave concurrent runs
-- (same rationale as the tutorial scripts' tutorialLiveHostiles).
escortState = {};

function onCreate(starterPlayer, contentArea, director)
	escortState[starterPlayer.actorId] = {
		done = false,        -- terminal latch (arrival signalled OR failed)
		wpIndex = 1,         -- next TRAIL breadcrumb (re-seated to nearest on first tick)
		startTick = nil,     -- latched on the first onUpdate tick (post-warp)
		lastBarkTick = nil,
		lastNear = 0,        -- live-mob count near the party last tick (wave-outcome beat)
		reminded20 = false,
		reminded10 = false,
		reminded5 = false,
		sawEscort = false,   -- Sisipu observed alive at least once (death detection)
		waveEscortFought = false,   -- Sisipu engaged during the current wave (thank-you variant)
		mobsLive = {},       -- actorId → true for position-synced live biters (defeat announcements)
		mobsEngaged = {},    -- actorId → true once "<mob> is engaged." was announced
		ringActorId = nil,   -- minimap-halo object id (re-acquired from the roster per tick)
	};

	-- Zone-128 route spawns (seed/070, re-homed one-per-ambush-point by
	-- seed/072). Rows 16-24 are the superseded zone-129 Skull Valley set,
	-- kept for rollback — flip these ids back to 16..24 alongside
	-- man0l1.lua's zone pin to roll back.
	sisipu = GetWorldManager().SpawnBattleNpcById(25, contentArea);
	local mobs = {};
	for bnpcId = 26, 33 do
		table.insert(mobs, GetWorldManager().SpawnBattleNpcById(bnpcId, contentArea));
	end

	-- Minimap duty halo (see ESCORT_RING_CLASS): spawned at Sisipu's
	-- seed position (seed/070 bnpc 25) and MoveTo'd alongside her every
	-- couple of ticks in onUpdate. Cross-tick userdata handles are dead
	-- (each tick drains a fresh command queue — lua/mod.rs
	-- call_content_on_update), so only the ID is stored; director
	-- membership puts the ring in the onUpdate monster roster (kind
	-- Npc — ticker.rs build_content_rosters) where each tick re-acquires
	-- a queue-bound handle. The combat loop skips it by this id.
	-- Teardown despawn is automatic (spawned_actor_ids tracking in
	-- apply_spawn_actor).
	local ring = contentArea:SpawnActor(ESCORT_RING_CLASS, "escortAreaRange", -49.0, 36.43, 162.0, 0);
	escortState[starterPlayer.actorId].ringActorId = ring.actorId;

	-- Active MainState so Sisipu stands and the ankle biters render
	-- hostile (tutorial-fight pattern).
	sisipu:ChangeState(2);
	for i = 1, #mobs do
		mobs[i]:ChangeState(2);
	end

	-- NO party-add here. onCreate runs PRE-warp, and a currentParty:AddMember
	-- broadcasts a content/party group trio (0x017C/D/F/E) referencing Sisipu
	-- before the client has spawned her — a pre-kick divergence from pmeteor's
	-- working SEQ_005 warp burst (which emits no party trio pre-kick). The HUD
	-- HP-bar party-add is a cosmetic follow-up to wire post-warp once the warp
	-- itself is solid. (Garlemald-Server #46.)

	-- The PLAYER keeps the tutorial-style 1-HP floor (retail wipes are
	-- a rez-and-retry; garlemald's death/return flow isn't wired for
	-- content yet). Sisipu takes REAL damage now — her death is the
	-- retail fail condition, detected in onUpdate (she leaves the
	-- live-only ally roster) → escortFail. (Garlemald-Server #46 —
	-- replaces the first cut's MinimumHpLock on her.)
	starterPlayer:SetMod(modifiersGlobal.MinimumHpLock, 1);

	director:AddMember(starterPlayer);
	director:AddMember(director);
	director:AddMember(sisipu);
	director:AddMember(ring);
	for i = 1, #mobs do
		director:AddMember(mobs[i]);
	end
end

-- Entry TEXT lives in onUpdate's first-sighting latch (round 9 — it
-- needs Sisipu's actor id for the 51005 param and runs post-ack): this
-- hook fires inside apply_do_zone_change_content immediately after the
-- bundle flush — ~4s BEFORE the client's RX 0x0007 (wire-proven
-- 2026-07-03 14:24: 0x0157 game messages shipped into the Now-Loading
-- gap crash the client). SetMusic is load-gap-SAFE — the bundle's own
-- zone-default SetMusic(60) ships in the same gap — and this 0x000C
-- lands after it, so the duty override wins. Restore is automatic on
-- every exit: the completion camp warp and the escortFail eject both
-- re-send SetMusic from the destination's DB row via
-- send_zone_in_bundle (design: scratchpad design_bgm). (#46, round 9.)
function onZoneIn(player, contentArea, director)
	player:ChangeMusic(ESCORT_DUTY_MUSIC);
end

function onDestroy()
end

local function dist2d(ax, az, bx, bz)
	local dx = ax - bx;
	local dz = az - bz;
	return math.sqrt(dx * dx + dz * dz);
end

-- Sisipu say-line delivery: quest-sheet text via the server-side
-- SendGameMessage mechanism (same shape as man0l1.lua's
-- seq007_endSequence 333/334 sends — text owner = the Man0l1 quest
-- actor). nil ids (unprobed bark slots) are skipped.
local function emitBark(owner, sayId)
	if (sayId == nil) then
		return;
	end
	local quest = owner:GetQuest("Man0l1");
	owner:SendGameMessage(quest, sayId, 0x20);
end

-- FAIL flow (Sisipu died / 30-minute expiry / confirmed leave-duty):
-- roll the quest back to SEQ_048 — its onStateChange re-arms the
-- ZEPHYR_TRIGGER push circle, so the duty is retryable from the gate —
-- unbind message, tear the instance down, eject to the gate.
-- (Garlemald-Server #46 — retail: failure ejects back to the gate and
-- the escort can be restarted.)
local function escortFail(owner, area, state)
	state.done = true;
	local quest = owner:GetQuest("Man0l1");
	quest:StartSequence(SEQ_048);
	owner:SendGameMessage(GetWorldMaster(), TEXT_UNBOUND_FROM_DUTY, 0x20);
	-- ContentFinished BEFORE the warp-out (QuestDirectorMan0l001
	-- teardown order); the eject is a public-area warp to the gate.
	area:ContentFinished();
	GetWorldManager():WarpToPublicArea(owner, -63.25, 33.15, 164.51, 0.8);
	-- Drain the director coroutine parked on waitForSignal
	-- ("escortComplete") so a stale park can't double-fire the arrival
	-- flow on a later retry (drain_signal wakes EVERY parked coroutine
	-- with that name). The woken coroutine's kickEventContinue emits a
	-- KickEvent for the just-wiped director actor — the client drops
	-- kicks for wiped actors (wire-proven, session 53943), the
	-- coroutine parks on _WAIT_EVENT and is displaced by the player's
	-- next event; crucially its StartSequence(SEQ_055) tail sits AFTER
	-- the never-answered callClientFunction, so no quest state moves.
	-- (Garlemald-Server #46 — known-limitation breadcrumb: a Rust-side
	-- scheduler purge on ContentFinished would retire this dance.)
	sendSignal("escortComplete");
end

function onUpdate(tick, area)
	if not area then return end
	local players = area:GetPlayers()
	local mobs    = area:GetMonsters()   -- live-only (dead filtered)
	local allies  = area:GetAllies()

	local owner = nil
	for player in players do
		if player then owner = owner or player end
	end
	if not owner then return end
	local state = escortState[owner.actorId]
	if not state or state.done then return end

	-- NO cross-VM go-latch (round 7d). The director and this content
	-- script load into SEPARATE Lua VMs (the vm_cache is keyed by script
	-- path), so a `man0l1EscortGo` global the director set in ITS VM is
	-- invisible here — the previous gate read this VM's copy (always
	-- false) and Sisipu never moved (wire-proven 20:50: zero
	-- MoveActorToPosition). The ticker already parks the content driver
	-- on `content_warp_acked` (the client's post-warp zone-in echo), and
	-- the veil is cleared by the director's questBaseRewardSeting ~1s in,
	-- so the escort simply runs from the first post-ack tick — a couple
	-- of Sisipu's steps during the fade are invisible. (Garlemald-Server
	-- #46, round 7d.)

	-- ---- Timer (retail: 30-minute limit) ----
	-- `tick` is the ticker's monotonic 500 ms frame counter; latch the
	-- first faded-in frame and measure elapsed from there. Reminder
	-- lines reuse 25018 "There ~is/are~ <N> ~minute/minutes~
	-- remaining." with the minutes as the message param (sheet-decoded;
	-- the round-7d boundary-family candidate 34112 was wrong).
	state.startTick = state.startTick or tick;
	local elapsed = tick - state.startTick;
	if elapsed >= ESCORT_LIMIT_TICKS then
		escortFail(owner, area, state);
		return;
	end
	if not state.reminded20 and elapsed >= REMIND_20MIN_TICKS then
		state.reminded20 = true;
		owner:SendGameMessage(GetWorldMaster(), TEXT_TIME_REMAINING, 0x20, 20);
	end
	if not state.reminded10 and elapsed >= REMIND_10MIN_TICKS then
		state.reminded10 = true;
		owner:SendGameMessage(GetWorldMaster(), TEXT_TIME_REMAINING, 0x20, 10);
	end
	if not state.reminded5 and elapsed >= REMIND_5MIN_TICKS then
		state.reminded5 = true;
		owner:SendGameMessage(GetWorldMaster(), TEXT_TIME_REMAINING, 0x20, 5);
	end

	-- ---- Sisipu death = fail (rosters are live-only, so a dead
	-- escort simply vanishes from GetAllies) ----
	local escort = allies[1]
	if escort then
		if not state.sawEscort then
			-- First live sighting = the duty is underway (the ticker
			-- only runs post-content_warp_acked, so the client has
			-- loaded — 0x0157 is safe). Retail entry order: journal →
			-- 34108 (Rust-side, rides the content-warp path) →
			-- protect → bound → timer, then Sisipu's send-off. 51005
			-- resolves "<displayName>" from the actor-id param — this
			-- is why the banner set lives HERE and not in the
			-- director, which has no handle on her.
			owner:SendGameMessage(GetWorldMaster(), TEXT_PROTECT_SISIPU, 0x20, escort);
			owner:SendGameMessage(GetWorldMaster(), TEXT_BOUND_BY_DUTY, 0x20);
			owner:SendGameMessage(GetWorldMaster(), TEXT_TIME_REMAINING, 0x20, ESCORT_LIMIT_MINUTES);
			emitBark(owner, BARKS.dutyStart);
		end
		state.sawEscort = true;
	elseif state.sawEscort then
		escortFail(owner, area, state);
		return;
	else
		-- Pre-onCreate tick (roster not populated yet) — wait.
		return;
	end

	-- ---- Ambush points ----
	-- The eight seeded biters sit ONE PER AMBUSH POINT at even ninths of
	-- the recorded arc (~130-160 units apart — seed/072; screenshots show
	-- a single biter per location), so the engage radius activates exactly
	-- the point whose stretch the party has reached: the live ankle biter
	-- inside ENGAGE_RADIUS of the escort or the player pulls alone;
	-- Sisipu fights back like the tutorial allies do.
	-- Round 7h: skip actors whose roster position is still unsynced
	-- (0,0,0) — on the spawn tick every distance reads ~0, so the wave-1
	-- mobs (then a 3-mob cluster) engaged the player AND Sisipu instantly
	-- from ~225 units away (log 23:05:49: ActorEngage rows at
	-- content-spawn time = the "invisible enemies" report). A real
	-- position never sits exactly at origin in zone 128.
	local function positionLive(a)
		return a and not (a.positionX == 0 and a.positionY == 0 and a.positionZ == 0);
	end
	if not positionLive(escort) or not positionLive(owner) then
		return;
	end

	local nearLive = 0
	local anyEngaged = false
	local nearestMob, nearestD = nil, math.huge
	-- The minimap-halo ring object rides the same director roster
	-- (kind Npc → monsters — ticker.rs build_content_rosters); pick its
	-- fresh queue-bound handle out and keep it clear of combat logic.
	local ring = nil
	local liveIds = {}
	for i = 1, #mobs do
		local mob = mobs[i]
		if mob and state.ringActorId ~= nil and mob.actorId == state.ringActorId then
			ring = mob;
		elseif mob and positionLive(mob) then
			liveIds[mob.actorId] = mob;
			local dMob = math.min(
				dist2d(mob.positionX, mob.positionZ, escort.positionX, escort.positionZ),
				dist2d(mob.positionX, mob.positionZ, owner.positionX, owner.positionZ))
			if dMob <= HOLD_RADIUS then
				nearLive = nearLive + 1
			end
			if mob:IsEngaged() then
				anyEngaged = true
				-- "The ankle biter is engaged." — announced once per mob on
				-- its first engaged tick. The mob itself is the param: an
				-- actor handle goes out as a type-6 actor reference, which
				-- the client resolves for the row's subject (a plain integer
				-- actor id, or a DispId sender, left it blank). (#202)
				if not state.mobsEngaged[mob.actorId] then
					state.mobsEngaged[mob.actorId] = true;
					owner:SendGameMessage(GetWorldMaster(), TEXT_MOB_ENGAGED, 0x20, mob);
				end
			end
			if dMob < nearestD then
				nearestMob, nearestD = mob, dMob
			end
			if dMob <= ENGAGE_RADIUS and not mob:IsEngaged() then
				allyGlobal.EngageTarget(mob, (i % 2 == 0) and escort or owner)
			end
		end
	end
	-- "The ankle biter is defeated." — the roster is live-only, so a
	-- previously-seen biter vanishing from it died this tick. Its last
	-- handle is the subject; the corpse is still on the client this tick.
	-- (#202)
	for id, lastSeen in pairs(state.mobsLive) do
		if not liveIds[id] then
			owner:SendGameMessage(GetWorldMaster(), TEXT_MOB_DEFEATED, 0x20, lastSeen);
		end
	end
	state.mobsLive = liveIds;

	-- Minimap halo follows Sisipu EVERY tick. Re-home the invisible
	-- range-marker ring onto her current position HERE — before all of
	-- the movement early-returns below (hold-while-contested, arrival,
	-- player-leash) — so the halo tracks her while she walks AND while she
	-- is pinned fighting an ambush wave. (The re-home used to sit AFTER
	-- those returns, so the instant the player fell >PLAYER_LEASH behind
	-- to fight, onUpdate returned early and the halo froze off-center for
	-- the whole fight.) The native range marker is actor-attached, so one
	-- 0x00CF (MoveTo) drags the halo on the client's minimap;
	-- RING_MOVE_TICKS=1 re-homes per frame, so it also snaps onto her on
	-- the first position-synced tick instead of lagging ~6 s from spawn.
	-- (Garlemald-Server #199.)
	-- moveState 2 (running), NOT 0: the wire enum is 0=standing /
	-- 1=walking / 2=running (UpdatePlayerPositionPacket.cs:33), and the
	-- client GLIDES a state-0 0x00CF instead of snapping — re-targeted
	-- every 500 ms at glide pace the halo fell ever further behind a
	-- running Sisipu until it left the minimap entirely (live
	-- 2026-07-05: server re-homes glued to her coords, client marker
	-- lagged then vanished). State 2 is the exact pipeline Sisipu's own
	-- every-tick MoveTo renders perfectly with. (#199 round 2.)
	if ring ~= nil and (state.lastRingTick == nil or tick - state.lastRingTick >= RING_MOVE_TICKS) then
		state.lastRingTick = tick;
		ring:MoveTo(escort.positionX, escort.positionY, escort.positionZ, 0.0, 2);
	end

	-- Sisipu joins the fight against the NEAREST in-range mob only.
	-- (Round 7g: this used to target mobs[1] — the first ROSTER entry,
	-- which could be a far unstreamed cluster hundreds of units away →
	-- ranged combat against an INVISIBLE attacker, the 8/8 report.)
	if not escort:IsEngaged() and nearestMob ~= nil and nearestD <= ENGAGE_RADIUS then
		allyGlobal.EngageTarget(escort, nearestMob)
	end

	-- Ambush-outcome beat: the contested count dropping back to zero =
	-- this ambush point was cleared this tick. Variant by how the
	-- fight went for Sisipu (HP snapshot refreshed per tick —
	-- build_content_rosters, #46 escort R4a): badly hurt → "Ow ow
	-- ow..."; scraped → "Phew..."; she had to fight herself → "Thank
	-- the Navigator!"; untouched → "Good show!".
	if escort:IsEngaged() then
		state.waveEscortFought = true;
	end
	if nearLive == 0 and state.lastNear > 0 then
		local hp, maxHp = escort:GetHP(), escort:GetMaxHP();
		local ratio = (maxHp > 0) and (hp / maxHp) or 1.0;
		if ratio <= HURT_HP_RATIO then
			emitBark(owner, BARKS.sisipuHurt);
		elseif ratio <= CLOSE_HP_RATIO then
			emitBark(owner, BARKS.waveClose);
		elseif state.waveEscortFought then
			emitBark(owner, BARKS.waveThanks);
		else
			emitBark(owner, BARKS.waveClean);
		end
		state.waveEscortFought = false;
	end
	state.lastNear = nearLive;

	-- ---- Hold while contested (round 7g: she stays PUT until every
	-- enemy of the active gate is dead — any live mob engaged with the
	-- party, or still lurking inside the hold radius, pins her) ----
	if nearLive > 0 or anyEngaged or escort:IsEngaged() then
		return
	end

	-- ---- Arrival ----
	-- The PLAYER reaching the lighthouse-approach point ends the duty
	-- (the journal objective marker points them there). Sisipu is leashed
	-- to the player below, so she is always in range too. → arrival bark
	-- → the director's kickEventContinue machinery (processEvent605 echo →
	-- SEQ_055 → camp warp) takes over.
	if dist2d(owner.positionX, owner.positionZ, ARRIVAL_GOAL.x, ARRIVAL_GOAL.z) <= ARRIVAL_RADIUS then
		state.done = true;
		emitBark(owner, BARKS.arrival);
		sendSignal("escortComplete");
		return;
	end

	-- ---- Sisipu TRACES the player's recorded footsteps (round 7f) ----
	-- She advances breadcrumb-to-breadcrumb along TRAIL — every point is
	-- ground the player actually walked (real terrain Y), so she cannot
	-- straight-line into the ocean or float (the 7e player-relative
	-- follow cut corners off walkable land). She waits when the player
	-- falls behind the leash. The recording now reaches the lighthouse
	-- itself (round 8), so the 7e player-relative follow past the last
	-- breadcrumb is only a safety fallback (arrival normally fires
	-- first). (Garlemald-Server #46, rounds 7f/8.)
	local dPlayer = dist2d(escort.positionX, escort.positionZ, owner.positionX, owner.positionZ);

	-- One-time: start from the trail point nearest her spawn (the first
	-- breadcrumb is the warp-in spot behind her seed position).
	if not state.trailInit then
		state.trailInit = true;
		local bestI, bestD = 1, math.huge;
		for i = 1, #TRAIL do
			local d = dist2d(escort.positionX, escort.positionZ, TRAIL[i].x, TRAIL[i].z);
			if d < bestD then bestI, bestD = i, d; end
		end
		state.wpIndex = bestI;
	end

	if dPlayer > PLAYER_LEASH then
		-- Player lagging (fight, detour) — she holds and (rate-limited)
		-- chides rather than walking off; the trail keeps her honest, the
		-- leash keeps her helpful.
		if state.lastBarkTick == nil or tick - state.lastBarkTick >= BARK_INTERVAL_TICKS then
			state.lastBarkTick = tick;
			emitBark(owner, BARKS.dawdle);
		end
		return;
	end

	if state.wpIndex <= #TRAIL then
		-- ON-TRAIL: step toward the next recorded breadcrumb at ITS
		-- recorded ground Y. RUN pace (moveState 2) — the 7e walk was
		-- reported too slow to keep up with a running player.
		-- Skip THROUGH every breadcrumb already within radius in one
		-- tick (round 7g: overlapping recorded points used to cost one
		-- idle tick each — she stood in place burning time), then move
		-- toward the first genuinely-ahead one this same tick.
		local wp, d;
		repeat
			wp = TRAIL[state.wpIndex];
			d = wp and dist2d(escort.positionX, escort.positionZ, wp.x, wp.z) or nil;
			if d ~= nil and d <= WAYPOINT_RADIUS then
				state.wpIndex = state.wpIndex + 1;
			end
		until wp == nil or d == nil or d > WAYPOINT_RADIUS or state.wpIndex > #TRAIL;
		if wp ~= nil and d ~= nil and d > WAYPOINT_RADIUS then
			local dx = (wp.x - escort.positionX) / d;
			local dz = (wp.z - escort.positionZ) / d;
			local step = math.min(RUN_STEP, d);
			escort:MoveTo(escort.positionX + dx * step, wp.y, escort.positionZ + dz * step,
				math.atan(dx, dz), 2);
		end
	else
		-- TRAIL EXHAUSTED: player-relative follow (7e) as a safety
		-- fallback only — the trail now ends at the lighthouse, inside
		-- the arrival radius, so this branch is normally unreachable.
		-- Her Y comes from the player's real ground height.
		local py = owner.positionY;
		local last = state.lastOwnerX and { x = state.lastOwnerX, z = state.lastOwnerZ } or nil;
		local moved = last and dist2d(owner.positionX, owner.positionZ, last.x, last.z) or 0;
		if dPlayer > LEAD_DISTANCE and moved > MOVE_EPSILON then
			local d = math.max(dPlayer, 0.001);
			local dx = (owner.positionX - escort.positionX) / d;
			local dz = (owner.positionZ - escort.positionZ) / d;
			local step = math.min(RUN_STEP, dPlayer);
			escort:MoveTo(escort.positionX + dx * step, py, escort.positionZ + dz * step,
				math.atan(dx, dz), 2);
		end
	end
	state.lastOwnerX = owner.positionX;
	state.lastOwnerZ = owner.positionZ;

	-- (Minimap-halo re-home hoisted above the movement early-returns —
	-- see the block after the mob loop. #199.)

	-- Guidance bark ("Oschon's Torch is due south...") every ~20 s while
	-- escorting — man0l1 sheet row 283.
	if state.lastBarkTick == nil or tick - state.lastBarkTick >= BARK_INTERVAL_TICKS then
		state.lastBarkTick = tick;
		emitBark(owner, BARKS.guidance);
	end
end

-- Leave-duty teardown (the commandContent confirmed-leave, driven from
-- the Rust command surface — there is no Lua command script for it):
-- same eject-and-retry flow as a timeout/death fail. Rust can fire this
-- via call_content_hook(script, "onAbort", ...) with the same
-- (player, contentArea, director) shape as onZoneIn.
-- (Garlemald-Server #46 — coordinate with the R5 leave-duty track.)
function onAbort(player, contentArea, director)
	local state = escortState[player.actorId];
	if state == nil then
		state = { done = false };
		escortState[player.actorId] = state;
	end
	if state.done then
		return;
	end
	escortFail(player, contentArea, state);
end
