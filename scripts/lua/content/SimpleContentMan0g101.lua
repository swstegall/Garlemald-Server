require ("global")
require ("modifiers")
-- onUpdate drives mob engagement through allyGlobal.EngageTarget;
-- without this require the global is nil and every tick errors
-- (swallowed by the ticker drain). Same shape as SimpleContentMan0l101.
require ("ally")
-- SEQ_* constants + FLAG_ESCORT_HANDOFF (proven loadable outside the
-- quest runtime — QuestDirectorMan0g101 requires it too).
require ("quests/man/man0g1")

-- Man0g1 "Souls Gone Wild" SEQ_065 — the White Wolf Gate → Lifemend
-- Stump escort duty (Garlemald-Server #41). Net-new content: upstream
-- pmeteor shipped the trigger arm commented out ("DO ESCORT DUTY HERE
-- ... For now just skip the sequence") and startMan0g1Content existed
-- nowhere — this is a re-instantiation of the proven man0l1 escort
-- machinery (SimpleContentMan0l101.lua; #46/#199) with Gridania
-- parameters.
--
-- Retail shape (Mirke Loremonger transcript + the 7-part playthrough
-- mined for #41): POWLE leads and SANSA tags along ("gather your little
-- friends" — only these two speak on the walk); ankle-biter (chigoe)
-- ambushes en route — the same mob family as the Limsa escort ("The
-- ankle biter is engaged." on film in Gridania too); "There are 20
-- minutes remaining." attested mid-walk (the man0l1 30-minute ladder);
-- arrival at the stump fires the duty release and hands off to the
-- Hermit of the Wood scene (the director's processEvent180 →
-- SEQ_070 stump beats, already ported).
--
-- The kids DO NOT fight (retail bark: "Do something! You are an
-- adventurer, aren't you?") — ambushers engage the PLAYER; the kids can
-- still be hit (retail "Ouch! Ow! Ooouch!" barks) and EITHER kid dying
-- fails the duty.

-- worldMaster duty-block text ids — sheet-decoded during the man0l1
-- escort work (direct client text-sheet DAT decode; see
-- quests/man/man0l1.lua TEXT_*). Global rows, redeclared here because
-- this VM requires man0g1, not man0l1.
TEXT_PROTECT_ESCORTEE	= 51005;	-- "Protect <displayName> from harm."
TEXT_BOUND_BY_DUTY		= 50011;	-- "You are now bound by duty."
TEXT_UNBOUND_FROM_DUTY	= 50012;	-- "You are no longer bound by duty."
TEXT_TIME_REMAINING		= 25018;	-- "There ~is/are~ <N> ~minute/minutes~ remaining."
TEXT_MOB_ENGAGED		= 30120;	-- "<displayName> is engaged."
TEXT_MOB_DEFEATED		= 30121;	-- "<displayName> is defeated."

-- Escort pacing / geometry — copied verbatim from the man0l1 leg (the
-- 500 ms tick, radii, and the render-before-fight invariant:
-- ENGAGE_RADIUS (18) + PLAYER_LEASH (15) stays below
-- INSTANCE_STREAM_RADIUS (50, world_manager.rs), as does the
-- controller's MAX_DETECT_DISTANCE (20)).
RUN_STEP = 3.4;              -- units per 500 ms tick (run pace)
HOLD_RADIUS = 28.0;          -- live mob within this of a kid/player → hold the walk
ENGAGE_RADIUS = 18.0;        -- mob pulls onto the player inside this
PLAYER_LEASH = 15.0;         -- Powle > this from the player → hold and chide
WAYPOINT_RADIUS = 4.0;       -- close enough to the lead point → next breadcrumb
ARRIVAL_RADIUS = 20.0;       -- player+Powle inside this of the goal → arrival
FOLLOW_GAP = 6.0;            -- Sansa trails Powle by up to this before stepping
TICKS_PER_SECOND = 2;
ESCORT_LIMIT_MINUTES = 30;   -- "There are 20 minutes remaining." on film ⇒ the 30-min ladder
ESCORT_LIMIT_TICKS = ESCORT_LIMIT_MINUTES * 60 * TICKS_PER_SECOND;
REMIND_20MIN_TICKS = 10 * 60 * TICKS_PER_SECOND;
REMIND_10MIN_TICKS = 20 * 60 * TICKS_PER_SECOND;
REMIND_5MIN_TICKS  = 25 * 60 * TICKS_PER_SECOND;
BARK_INTERVAL_TICKS = 40;    -- march bark every ~20 s
ESCORT_DUTY_MUSIC = 37;      -- "Daring Dalliances" — the 1.x duty theme (kept from man0l1)
KID_HURT_BARK_COOLDOWN = 10; -- ticks between "Ouch!" barks when a kid is taking hits
-- Minimap duty halo — the invisible ContentPrivateAreaRange object
-- (class 1290003, global gamedata row from seed/073). Rides Powle;
-- re-homed at moveState 2 every frame (#199 round 2 semantics).
ESCORT_RING_CLASS = 1290003;
RING_MOVE_TICKS = 1;

-- ============ ESCORT MOVEMENT + (optional) TRAIL RECORDING ============
-- Retail model: the PLAYER leads to the Lifemend Stump and the two kids
-- FOLLOW (player-relative, man0l1 round-7e) — they never lead. onUpdate
-- moves Powle to close the gap to the player at run pace; Sansa trails
-- Powle. The ankle biters sit evenly along the recorded route (seed/085).
--
-- RECORD_MODE is a one-shot CAPTURE toggle (movement is player-follow
-- either way): true rides the invisible halo ring on the PLAYER every
-- tick so its MoveActorToPosition log line (target
-- map_server::runtime::quest_apply, on at the default map_server=debug)
-- records the exact walked ground — extract the ring's lines from
-- logs/map-server.log, dedupe + downsample, paste into TRAIL / ARRIVAL_GOAL
-- / seed the biters. false = normal play (the ring rides Powle as the
-- minimap duty halo). The route below WAS captured this way (2026-07-06,
-- a 1004-unit gate→stump arc).
RECORD_MODE = false;
LEAD_DISTANCE = 5.0;         -- Powle closes to within this of the player (the follow gap)
MOVE_EPSILON = 0.3;          -- (reserved) minimum player displacement to count as "walking"

-- Same ambusher family as the Limsa escort: chigoe class 2205603,
-- displayNameId 3205603 ("ankle biter"). The engaged/defeated rows
-- 30120/30121 take their subject from the DispId-sender wire family
-- (see #199/#202 — an actor-id LuaParam renders a blank subject).
ANKLE_BITER_DISPLAY_ID = 3205603;

-- ============ RECORDED ROUTE (Garlemald-Server #41) ============
-- The player's real walked ground, White Wolf Gate → Lifemend Stump
-- (recorded 2026-07-06 via RECORD_MODE, deduped + downsampled to ~18u; a
-- 1004-unit arc). Reference + biter/arrival anchor only: the kids FOLLOW
-- the player so they do NOT trace this, but the ankle biters are spaced
-- along it (seed/085) and ARRIVAL_GOAL is its last point.
TRAIL = {
	{ x = -194.73, y = 3.54, z = -1021.33 },
	{ x = -188.77, y = 3.68, z = -1003.75 },
	{ x = -187.11, y = 3.99, z = -984.60 },
	{ x = -186.58, y = 6.13, z = -966.65 },
	{ x = -188.28, y = 5.42, z = -947.26 },
	{ x = -193.27, y = 4.04, z = -928.21 },
	{ x = -195.48, y = 3.72, z = -909.25 },
	{ x = -185.13, y = 3.42, z = -892.53 },
	{ x = -187.31, y = 3.57, z = -872.56 },
	{ x = -185.66, y = 4.64, z = -852.29 },
	{ x = -187.28, y = 4.33, z = -835.64 },
	{ x = -203.65, y = 4.44, z = -824.99 },
	{ x = -217.06, y = 4.43, z = -808.32 },
	{ x = -234.92, y = 5.59, z = -810.88 },
	{ x = -248.56, y = 4.39, z = -824.51 },
	{ x = -263.54, y = 4.89, z = -839.57 },
	{ x = -282.43, y = 4.20, z = -835.31 },
	{ x = -299.81, y = 4.57, z = -832.41 },
	{ x = -319.60, y = 7.55, z = -839.92 },
	{ x = -336.40, y = 5.20, z = -852.87 },
	{ x = -352.59, y = 4.00, z = -864.27 },
	{ x = -369.09, y = 4.70, z = -876.71 },
	{ x = -380.83, y = 4.18, z = -894.44 },
	{ x = -396.64, y = 5.19, z = -902.42 },
	{ x = -415.75, y = 4.09, z = -899.02 },
	{ x = -433.36, y = 1.55, z = -900.18 },
	{ x = -454.00, y = -0.69, z = -895.00 },
	{ x = -473.40, y = 3.41, z = -893.16 },
	{ x = -492.48, y = 4.73, z = -897.51 },
	{ x = -510.17, y = 7.33, z = -904.99 },
	{ x = -527.29, y = 5.18, z = -917.99 },
	{ x = -543.59, y = 4.00, z = -928.30 },
	{ x = -558.86, y = 5.26, z = -940.53 },
	{ x = -570.57, y = 4.30, z = -952.23 },
	{ x = -581.78, y = 4.75, z = -968.29 },
	{ x = -600.16, y = 4.16, z = -963.35 },
	{ x = -616.74, y = 3.72, z = -956.32 },
	{ x = -637.37, y = 3.98, z = -953.99 },
	{ x = -646.07, y = 3.75, z = -970.95 },
	{ x = -639.33, y = 4.02, z = -990.88 },
	{ x = -633.32, y = 7.12, z = -1011.26 },
	{ x = -638.98, y = 11.53, z = -1029.27 },
	{ x = -638.20, y = 19.00, z = -1049.53 },
	{ x = -635.46, y = 21.42, z = -1067.89 },
	{ x = -640.57, y = 23.03, z = -1085.74 },
	{ x = -655.57, y = 22.68, z = -1100.03 },
	{ x = -676.77, y = 21.01, z = -1096.99 },
	{ x = -697.41, y = 22.81, z = -1091.66 },
	{ x = -714.05, y = 22.86, z = -1084.68 },
	{ x = -734.59, y = 20.01, z = -1088.34 },
	{ x = -755.16, y = 22.64, z = -1092.80 },
	{ x = -756.77, y = 22.77, z = -1092.33 },
};
-- Destination = the Lifemend Stump (the last recorded point). The player
-- reaching it, kids in tow, ends the duty and hands off to the Hermit of
-- the Wood (SEQ_070).
ARRIVAL_GOAL = { x = -756.77, y = 22.77, z = -1092.33 };
-- Duty warp-in = the EXACT White Wolf Gate coords where the "Duty calls"
-- dialog fires (seed/081 stand-and-mark). The Gridania region is one
-- seamless coordinate space, so this gate point is valid inside the
-- zone-150 instance. The halo ring spawns here and the kids (seed/084)
-- beside it, so the escort BEGINS at the gate. Keep in sync with
-- man0g1.lua startMan0g1Content.
WARP_IN = { x = -194.73, y = 3.54, z = -1021.33, rot = -1.642 };
-- Fail/eject returns the player to the gate warp-in.
GATE_EJECT = { x = WARP_IN.x, y = WARP_IN.y, z = WARP_IN.z, rot = WARP_IN.rot };

-- March/outcome barks — retail lines from the Mirke Loremonger
-- transcript (speakers attested: Powle leads, Sansa seconds). The
-- man0g1 QUEST-sheet say ids for these lines are NOT yet decoded (the
-- man0l1 ids came from a direct client text-sheet DAT decode — same
-- .le.lpb/XOR-0x73 recipe applies; TODO #41 follow-up). Until then
-- emitBark falls back to a literal General-log line so the retail text
-- reaches the player, localization pending.
BARKS = {
	dutyStart   = { id = nil, who = "Powle", text = "Let's go! Off to the Twelveswood!" },
	guidance    = { id = nil, who = "Sansa", text = "This is even more fun than I thought it would be." },
	waveCall    = { id = nil, who = "Powle", text = "What was that? Did you see something? Do something! You are an adventurer, aren't you?" },
	waveClear   = { id = nil, who = "Sansa", text = "You did it! You did it!" },
	waveResume  = { id = nil, who = "Powle", text = "Now back to the march!" },
	waveThanks  = { id = nil, who = "Powle", text = "It's a good thing you're with us!" },
	kidHurt1    = { id = nil, who = "Sansa", text = "Ouch! Ow! Ooouch!" },
	kidHurt2    = { id = nil, who = "Sansa", text = "What did I do to deserve this?" },
	-- Powle's PLAYER-FELL-BEHIND impatience line (Mirke Loremonger 28526).
	-- NOT "We're almost there. Come on, hurry!" — that (Mirke 28532) is the
	-- near-goal hurry-up, which lands on the nearGoal beat below.
	dawdle      = { id = nil, who = "Powle", text = "I want to go home... This isn't fun. You're not fun." },
	nearGoal    = { id = nil, who = "Powle", text = "It's just up ahead!" },
	arrival     = { id = nil, who = "Powle", text = "We're here! And it's all thanks to you." },
	-- Sansa seconds the arrival before the Hermit-of-the-Wood handoff
	-- (Mirke Loremonger 28538).
	arrivalSansa = { id = nil, who = "Sansa", text = "Look! The stump is just over there!" },
};

-- Per-player escort state, keyed by actor id (the VM is process-cached
-- per script path — a plain scalar would interleave concurrent runs).
escortState = {};

function onCreate(starterPlayer, contentArea, director)
	escortState[starterPlayer.actorId] = {
		done = false,
		wpIndex = 1,
		startTick = nil,
		lastBarkTick = nil,
		lastRingTick = nil,
		lastNear = 0,
		reminded20 = false,
		reminded10 = false,
		reminded5 = false,
		sawKids = false,      -- both kids observed alive at least once
		calledNearGoal = false,
		lastKidHurtTick = nil,
		powleHp = nil,        -- HP snapshots for the kid-hit barks
		sansaHp = nil,
		mobsLive = {},
		mobsEngaged = {},
		ringActorId = nil,
		powleId = nil,        -- roster re-acquisition keys
		sansaId = nil,
		trailInit = false,
	};

	-- Zone-150 provisional spawns (seed/074): 34 = Powle, 35 = Sansa
	-- (FighterAlly-scripted classes carrying the kids' real
	-- displayNameIds + populace appearances), 36-41 = ankle biters.
	local powle = GetWorldManager().SpawnBattleNpcById(34, contentArea);
	local sansa = GetWorldManager().SpawnBattleNpcById(35, contentArea);
	escortState[starterPlayer.actorId].powleId = powle.actorId;
	escortState[starterPlayer.actorId].sansaId = sansa.actorId;
	local mobs = {};
	for bnpcId = 36, 41 do
		table.insert(mobs, GetWorldManager().SpawnBattleNpcById(bnpcId, contentArea));
	end

	-- Minimap duty halo riding Powle (cross-tick userdata is dead — only
	-- the ID is stored; director membership puts the ring in the
	-- onUpdate monster roster where each tick re-acquires a queue-bound
	-- handle; teardown despawn is automatic via spawned_actor_ids).
	local ring = contentArea:SpawnActor(ESCORT_RING_CLASS, "escortAreaRange", WARP_IN.x, WARP_IN.y, WARP_IN.z, 0);
	escortState[starterPlayer.actorId].ringActorId = ring.actorId;

	-- Active MainState so the kids stand ready and the biters render
	-- hostile (tutorial-fight pattern). The kids never auto-attack —
	-- no engage is ever scripted FOR them (retail: they don't fight).
	-- Civilian idle (state 0) for the kids. They're WEAPONLESS child
	-- models: battle MainState (2) has no combat idle to resolve, so the
	-- client fell back to the bind pose — the T-pose seen live 2026-07-06.
	-- Only the biters (real monsters) take the hostile battle state.
	powle:ChangeState(0);
	sansa:ChangeState(0);
	for i = 1, #mobs do
		mobs[i]:ChangeState(2);
	end

	-- Player keeps the tutorial-style 1-HP floor (garlemald's
	-- death/return flow isn't wired for content). The KIDS take real
	-- damage — either kid dying is the retail fail condition, detected
	-- in onUpdate via the live-only ally roster.
	starterPlayer:SetMod(modifiersGlobal.MinimumHpLock, 1);

	director:AddMember(starterPlayer);
	director:AddMember(director);
	director:AddMember(powle);
	director:AddMember(sansa);
	director:AddMember(ring);
	for i = 1, #mobs do
		director:AddMember(mobs[i]);
	end
end

-- Load-gap-SAFE duty music override (the entry banners live in
-- onUpdate's first-sighting latch — 0x0157 messages shipped into the
-- Now-Loading gap crash the client; see SimpleContentMan0l101 onZoneIn).
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

-- Kid bark delivery. Decoded man0g1 quest-sheet say ids slot into
-- BARKS[*].id when probed (then this becomes the man0l1
-- SendGameMessage(quest, sayId, 0x20) shape); until then the retail
-- line lands as a literal General-log message so the beat is present.
local function emitBark(owner, bark)
	if (bark == nil) then
		return;
	end
	if (bark.id ~= nil) then
		local quest = owner:GetQuest("Man0g1");
		owner:SendGameMessage(quest, bark.id, 0x20);
	else
		owner:SendMessage(0x20, "", bark.who .. ": " .. bark.text);
	end
end

-- FAIL flow (a kid died / 30-minute expiry / confirmed leave-duty):
-- roll the quest back to SEQ_060 — its onStateChange re-arms the
-- GATE_TRIGGER push circle so the duty is retryable from the gate —
-- unbind message, tear the instance down, eject to the gate anchor.
local function escortFail(owner, area, state)
	state.done = true;
	local quest = owner:GetQuest("Man0g1");
	quest:StartSequence(SEQ_060);
	owner:SendGameMessage(GetWorldMaster(), TEXT_UNBOUND_FROM_DUTY, 0x20);
	-- ContentFinished BEFORE the warp-out (tutorial teardown order).
	area:ContentFinished();
	GetWorldManager():WarpToPublicArea(owner, GATE_EJECT.x, GATE_EJECT.y, GATE_EJECT.z, GATE_EJECT.rot);
	-- Drain the director coroutine parked on "escortComplete" so a
	-- stale park can't double-fire the arrival flow on a retry (the
	-- woken kickEventContinue targets a wiped director actor — the
	-- client drops it; man0l1-proven shape).
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

	-- ---- Timer (the man0l1 30-minute ladder; the 20-minute reminder
	-- is on film for this duty) ----
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

	-- ---- Kid roster + death-fail (rosters are live-only, so a dead
	-- kid simply vanishes from GetAllies) ----
	local powle, sansa = nil, nil;
	for i = 1, #allies do
		local ally = allies[i];
		if ally and ally.actorId == state.powleId then
			powle = ally;
		elseif ally and ally.actorId == state.sansaId then
			sansa = ally;
		end
	end
	if (powle ~= nil and sansa ~= nil) then
		if not state.sawKids then
			-- First live sighting = the duty is underway. Retail entry
			-- order (man0l1 wire shape): protect banner → bound-by-duty
			-- → timer. 51005 resolves "<displayName>" from the actor-id
			-- param — Powle fronts the pair on the walk.
			owner:SendGameMessage(GetWorldMaster(), TEXT_PROTECT_ESCORTEE, 0x20, powle.actorId);
			owner:SendGameMessage(GetWorldMaster(), TEXT_BOUND_BY_DUTY, 0x20);
			owner:SendGameMessage(GetWorldMaster(), TEXT_TIME_REMAINING, 0x20, ESCORT_LIMIT_MINUTES);
			emitBark(owner, BARKS.dutyStart);
		end
		state.sawKids = true;
	elseif state.sawKids then
		-- Either kid died → duty failed.
		escortFail(owner, area, state);
		return;
	else
		-- Pre-onCreate tick (roster not populated yet) — wait.
		return;
	end

	-- Skip actors whose roster position is still unsynced (0,0,0) — on
	-- the spawn tick every distance reads ~0 (the man0l1 round-7h
	-- invisible-enemies rule; a real position never sits at origin).
	local function positionLive(a)
		return a and not (a.positionX == 0 and a.positionY == 0 and a.positionZ == 0);
	end
	if not positionLive(powle) or not positionLive(owner) then
		return;
	end

	-- ---- Kid-hit barks (HP snapshot deltas; rate-limited) ----
	local powleHp = powle:GetHP();
	local sansaHp = (sansa ~= nil) and sansa:GetHP() or nil;
	if ((state.powleHp ~= nil and powleHp < state.powleHp)
			or (state.sansaHp ~= nil and sansaHp ~= nil and sansaHp < state.sansaHp)) then
		if (state.lastKidHurtTick == nil or tick - state.lastKidHurtTick >= KID_HURT_BARK_COOLDOWN) then
			state.lastKidHurtTick = tick;
			emitBark(owner, (tick % 2 == 0) and BARKS.kidHurt1 or BARKS.kidHurt2);
		end
	end
	state.powleHp = powleHp;
	state.sansaHp = sansaHp;

	-- ---- Ambushes: a live biter inside ENGAGE_RADIUS of a kid or the
	-- player pulls onto the PLAYER (the kids don't fight — retail "Do
	-- something! You are an adventurer, aren't you?"). ----
	local nearLive = 0
	local anyEngaged = false
	local ring = nil
	local liveIds = {}
	for i = 1, #mobs do
		local mob = mobs[i]
		if mob and state.ringActorId ~= nil and mob.actorId == state.ringActorId then
			ring = mob;
		elseif mob and positionLive(mob) then
			liveIds[mob.actorId] = true;
			local dMob = math.min(
				dist2d(mob.positionX, mob.positionZ, powle.positionX, powle.positionZ),
				dist2d(mob.positionX, mob.positionZ, owner.positionX, owner.positionZ))
			if dMob <= HOLD_RADIUS then
				nearLive = nearLive + 1
			end
			if mob:IsEngaged() then
				anyEngaged = true
				if not state.mobsEngaged[mob.actorId] then
					state.mobsEngaged[mob.actorId] = true;
					owner:SendGameMessageLocalizedDisplayName(GetWorldMaster(), TEXT_MOB_ENGAGED, 0x20, ANKLE_BITER_DISPLAY_ID);
					emitBark(owner, BARKS.waveCall);
				end
			end
			if dMob <= ENGAGE_RADIUS and not mob:IsEngaged() then
				allyGlobal.EngageTarget(mob, owner)
			end
		end
	end
	-- Defeat announcements: a previously-seen biter vanishing from the
	-- live-only roster died this tick.
	for id in pairs(state.mobsLive) do
		if not liveIds[id] then
			owner:SendGameMessageLocalizedDisplayName(GetWorldMaster(), TEXT_MOB_DEFEATED, 0x20, ANKLE_BITER_DISPLAY_ID);
		end
	end
	state.mobsLive = liveIds;

	-- Minimap halo follows Powle EVERY tick — hoisted above all the
	-- movement early-returns; moveState 2 (the client glides state-0
	-- moves and the halo falls off the minimap — #199 round 2).
	if ring ~= nil and (state.lastRingTick == nil or tick - state.lastRingTick >= RING_MOVE_TICKS) then
		state.lastRingTick = tick;
		if RECORD_MODE then
			-- TRAIL RECORDING: ride the halo on the PLAYER so its logged
			-- MoveActorToPosition line captures the exact ground you walked.
			-- The ring is the only Npc-kind actor (0x..B6..); the kids are
			-- BattleNpc (0x..B4..). Extract after the walk with e.g.
			--   grep 'MoveActorToPosition applied actor="0x..B6' logs/map-server.log
			-- Runs every tick, ABOVE the hold/leash/arrival early-returns,
			-- so the path is captured continuously — even while you fight.
			ring:MoveTo(owner.positionX, owner.positionY, owner.positionZ, 0.0, 2);
		else
			ring:MoveTo(powle.positionX, powle.positionY, powle.positionZ, 0.0, 2);
		end
	end

	-- Wave-outcome beat: contested count dropping back to zero = this
	-- ambush cleared. Retail celebration pair, then the resume call.
	if nearLive == 0 and state.lastNear > 0 then
		emitBark(owner, BARKS.waveClear);
		emitBark(owner, BARKS.waveResume);
	end
	state.lastNear = nearLive;

	-- ---- Hold while contested ----
	if nearLive > 0 or anyEngaged then
		return
	end

	-- ---- Arrival: the PLAYER reaches the stump (the kids follow, so they
	-- arrive in tow) → duty complete, hand off to the Hermit of the Wood. ----
	if dist2d(owner.positionX, owner.positionZ, ARRIVAL_GOAL.x, ARRIVAL_GOAL.z) <= ARRIVAL_RADIUS then
		state.done = true;
		emitBark(owner, BARKS.arrival);
		emitBark(owner, BARKS.arrivalSansa);
		sendSignal("escortComplete");
		return;
	end

	-- ---- The kids FOLLOW the player (retail: the PLAYER leads to the
	-- stump; the children tag along and never lead). Powle closes the gap
	-- to the player at run pace, at the player's real ground Y; Sansa
	-- trails Powle below. NO leash-hold — you walk freely and they keep up.
	-- (TRAIL above is the recorded route, kept for reference + the seed/085
	-- biter spacing; the kids do not trace it.) ----
	local dPlayer = dist2d(powle.positionX, powle.positionZ, owner.positionX, owner.positionZ);
	if dPlayer > LEAD_DISTANCE then
		local d = math.max(dPlayer, 0.001);
		local dx = (owner.positionX - powle.positionX) / d;
		local dz = (owner.positionZ - powle.positionZ) / d;
		local step = math.min(RUN_STEP, dPlayer);
		powle:MoveTo(powle.positionX + dx * step, owner.positionY, powle.positionZ + dz * step,
			math.atan(dx, dz), 2);
	end

	-- Near-goal call-out as the player closes on the stump.
	if not state.calledNearGoal
			and dist2d(owner.positionX, owner.positionZ, ARRIVAL_GOAL.x, ARRIVAL_GOAL.z) <= 40.0 then
		state.calledNearGoal = true;
		emitBark(owner, BARKS.nearGoal);
	end

	-- Sansa tags along behind Powle (she never leads and never fights).
	if (sansa ~= nil and positionLive(sansa)) then
		local dGap = dist2d(sansa.positionX, sansa.positionZ, powle.positionX, powle.positionZ);
		if (dGap > FOLLOW_GAP) then
			local dx = (powle.positionX - sansa.positionX) / dGap;
			local dz = (powle.positionZ - sansa.positionZ) / dGap;
			local step = math.min(RUN_STEP, dGap - FOLLOW_GAP * 0.5);
			sansa:MoveTo(sansa.positionX + dx * step, powle.positionY, sansa.positionZ + dz * step,
				math.atan(dx, dz), 2);
		end
	end

	-- March flavour bark every ~20 s while walking.
	if state.lastBarkTick == nil or tick - state.lastBarkTick >= BARK_INTERVAL_TICKS then
		state.lastBarkTick = tick;
		emitBark(owner, (tick % 3 == 0) and BARKS.waveThanks or BARKS.guidance);
	end
end

-- Leave-duty teardown (the commandContent confirmed-leave, driven from
-- the Rust command surface): same eject-and-retry flow as a
-- timeout/death fail.
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
