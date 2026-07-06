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

-- Same ambusher family as the Limsa escort: chigoe class 2205603,
-- displayNameId 3205603 ("ankle biter"). The engaged/defeated rows
-- 30120/30121 take their subject from the DispId-sender wire family
-- (see #199/#202 — an actor-id LuaParam renders a blank subject).
ANKLE_BITER_DISPLAY_ID = 3205603;

-- ============ PROVISIONAL TRAIL (Garlemald-Server #41) ============
-- Placeholder route: a short arc beside the Lifemend Stump (wiki marker
-- zone 150 ≈ (-800, 20, -1050); the SEQ_070 stump warp lands at
-- (-770.197, 23, -1086.209)). This exercises the complete duty pipeline
-- (spawn → walk → ambush → arrival → director handoff) but is NOT the
-- retail route. REPLACE with breadcrumbs decoded from a recorded player
-- walk (White Wolf Gate → stump; inbound 0x00CA positions — the man0l1
-- round-7f recipe: "player 0x00CA packets are a free navmesh
-- substitute"). Trail-recording round tracked in #41.
TRAIL = {
	{ x = -700.00, y = 21.00, z = -1000.00 },
	{ x = -718.00, y = 21.00, z = -1014.00 },
	{ x = -736.00, y = 21.50, z = -1028.00 },
	{ x = -748.00, y = 22.00, z = -1042.00 },
	{ x = -756.00, y = 22.00, z = -1052.00 },
};
ARRIVAL_GOAL = { x = -756.00, y = 22.00, z = -1052.00 };
-- Fail/eject anchor — the duty warp-in point (provisional White Wolf
-- Gate stand-in until the recorded walk supplies the real gate coords).
GATE_EJECT = { x = -700.0, y = 21.0, z = -1000.0, rot = 2.4 };

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
	dawdle      = { id = nil, who = "Powle", text = "We're almost there. Come on, hurry!" },
	nearGoal    = { id = nil, who = "Powle", text = "It's just up ahead!" },
	arrival     = { id = nil, who = "Powle", text = "We're here! And it's all thanks to you." },
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
	local ring = contentArea:SpawnActor(ESCORT_RING_CLASS, "escortAreaRange", TRAIL[1].x, TRAIL[1].y, TRAIL[1].z, 0);
	escortState[starterPlayer.actorId].ringActorId = ring.actorId;

	-- Active MainState so the kids stand ready and the biters render
	-- hostile (tutorial-fight pattern). The kids never auto-attack —
	-- no engage is ever scripted FOR them (retail: they don't fight).
	powle:ChangeState(2);
	sansa:ChangeState(2);
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
		ring:MoveTo(powle.positionX, powle.positionY, powle.positionZ, 0.0, 2);
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

	-- ---- Arrival: the PLAYER and Powle both inside the goal radius ----
	if (dist2d(owner.positionX, owner.positionZ, ARRIVAL_GOAL.x, ARRIVAL_GOAL.z) <= ARRIVAL_RADIUS
			and dist2d(powle.positionX, powle.positionZ, ARRIVAL_GOAL.x, ARRIVAL_GOAL.z) <= ARRIVAL_RADIUS) then
		state.done = true;
		emitBark(owner, BARKS.arrival);
		sendSignal("escortComplete");
		return;
	end

	-- ---- Powle traces the TRAIL breadcrumbs (recorded-ground recipe;
	-- provisional arc until the real walk is recorded). He waits when
	-- the player falls behind the leash. ----
	local dPlayer = dist2d(powle.positionX, powle.positionZ, owner.positionX, owner.positionZ);

	if not state.trailInit then
		state.trailInit = true;
		local bestI, bestD = 1, math.huge;
		for i = 1, #TRAIL do
			local d = dist2d(powle.positionX, powle.positionZ, TRAIL[i].x, TRAIL[i].z);
			if d < bestD then bestI, bestD = i, d; end
		end
		state.wpIndex = bestI;
	end

	if dPlayer > PLAYER_LEASH then
		if state.lastBarkTick == nil or tick - state.lastBarkTick >= BARK_INTERVAL_TICKS then
			state.lastBarkTick = tick;
			emitBark(owner, BARKS.dawdle);
		end
		return;
	end

	if state.wpIndex <= #TRAIL then
		-- ON-TRAIL: step toward the next breadcrumb at ITS recorded Y;
		-- skip THROUGH every breadcrumb already inside the radius in
		-- one tick.
		local wp, d;
		repeat
			wp = TRAIL[state.wpIndex];
			d = wp and dist2d(powle.positionX, powle.positionZ, wp.x, wp.z) or nil;
			if d ~= nil and d <= WAYPOINT_RADIUS then
				state.wpIndex = state.wpIndex + 1;
			end
		until wp == nil or d == nil or d > WAYPOINT_RADIUS or state.wpIndex > #TRAIL;
		if wp ~= nil and d ~= nil and d > WAYPOINT_RADIUS then
			local dx = (wp.x - powle.positionX) / d;
			local dz = (wp.z - powle.positionZ) / d;
			local step = math.min(RUN_STEP, d);
			powle:MoveTo(powle.positionX + dx * step, wp.y, powle.positionZ + dz * step,
				math.atan(dx, dz), 2);
			-- Near-goal call-out once the tail breadcrumb is in play.
			if state.wpIndex >= #TRAIL and not state.calledNearGoal then
				state.calledNearGoal = true;
				emitBark(owner, BARKS.nearGoal);
			end
		end
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
