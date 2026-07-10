require ("global")
require ("modifiers")
-- onUpdate drives mob engagement through allyGlobal.EngageTarget;
-- without this require the global is nil and every tick errors
-- (swallowed by the ticker drain). Same shape as the sibling escorts.
require ("ally")
-- SEQ_* constants (proven loadable outside the quest runtime).
require ("quests/man/man0u1")

-- Man0u1 SEQ_060 — the F'lhaminn escort, Gate of Nald → Camp Black
-- Brush (Garlemald-Server #53). Re-instantiation of the wire-proven
-- man0l1/man0g1 escort machinery (SimpleContentMan0l101/Man0g101)
-- with Ul'dah parameters.
--
-- Retail shape (both full playthroughs): a 30-minute duty on zone 170
-- — "You have entered an instance. / You are now bound by duty. /
-- Protect F'lhaminn from harm. / There are 30 minutes remaining." —
-- chinchilla ambushes that one-shot-die at opener levels (60-80 dmg
-- Light Slash kills, 511-650 EXP each on film); F'lhaminn barks
-- between waves (man0u1 QUEST-sheet texts 365-370/374/375, decoded
-- from csv/man0u1.csv); on her wounding, the retreat-and-retry fail
-- (371/372 — "let us beat the retreat... The camp will wait for us").
--
-- F'lhaminn FOLLOWS the player (the sibling player-leads model); the
-- route reference below is marker-anchored interpolation — gate
-- (-34.5, -68.9) → camp (34.8, -480.5), y from the seed-031 zone-170
-- anchors — flagged for the RECORD_MODE re-capture pass the siblings
-- got (#41/#46).

-- worldMaster duty-block text ids (sheet-decoded during the man0l1
-- escort work).
TEXT_PROTECT_ESCORTEE	= 51005;	-- "Protect <displayName> from harm."
TEXT_BOUND_BY_DUTY		= 50011;	-- "You are now bound by duty."
TEXT_UNBOUND_FROM_DUTY	= 50012;	-- "You are no longer bound by duty."
TEXT_TIME_REMAINING		= 25018;	-- "There ~is/are~ <N> ~minute/minutes~ remaining."
TEXT_MOB_ENGAGED		= 30120;	-- "<displayName> is engaged."
TEXT_MOB_DEFEATED		= 30121;	-- "<displayName> is defeated."

-- Escort pacing / geometry — the man0l1/man0g1 constants verbatim.
RUN_STEP = 3.4;
HOLD_RADIUS = 28.0;
ENGAGE_RADIUS = 18.0;
WAYPOINT_RADIUS = 4.0;
ARRIVAL_RADIUS = 20.0;
LEAD_DISTANCE = 5.0;
TICKS_PER_SECOND = 2;
ESCORT_LIMIT_MINUTES = 30;   -- "There are 30 minutes remaining." on film
ESCORT_LIMIT_TICKS = ESCORT_LIMIT_MINUTES * 60 * TICKS_PER_SECOND;
REMIND_20MIN_TICKS = 10 * 60 * TICKS_PER_SECOND;
REMIND_10MIN_TICKS = 20 * 60 * TICKS_PER_SECOND;
REMIND_5MIN_TICKS  = 25 * 60 * TICKS_PER_SECOND;
BARK_INTERVAL_TICKS = 40;
ESCORT_DUTY_MUSIC = 37;
ESCORT_RING_CLASS = 1290003; -- minimap duty halo (seed/073 gamedata row)
RING_MOVE_TICKS = 1;
RECORD_MODE = false;         -- one-shot trail-capture toggle (see Man0g101)

-- Chinchilla — class 2204010, displayNameId 3204010 (bahamut
-- gamedata; the retail combat log names "chinchilla" throughout).
CHINCHILLA_DISPLAY_ID = 3204010;

-- Route reference: gate → camp, marker-anchored linear interpolation
-- (y anchors: gate ≈183, camp = 201 from the seed-031 zone-170 rows).
-- Reference + ambush/arrival anchor only — F'lhaminn follows the
-- player, she does not trace it.
ARRIVAL_GOAL = { x = 34.8, y = 201.0, z = -480.5 };
WARP_IN = { x = -34.5, y = 183.0, z = -68.9, rot = -3.0 };
GATE_EJECT = { x = WARP_IN.x, y = WARP_IN.y, z = WARP_IN.z, rot = WARP_IN.rot };

-- F'lhaminn's march/outcome barks — man0u1 QUEST-sheet say ids (all
-- verbatim on film; SendGameMessage against the quest sheet is the
-- man0l1 seq007_endSequence delivery shape).
SAY_DANGERS_AHEAD  = 365; -- "There may be dangers along the way..."
SAY_NOT_TOO_STRONG = 366; -- "It doesn't appear to be too strong. What say you?"
SAY_WELL_DONE      = 367; -- "Well done! I expected nothing less."
SAY_AMAZING        = 368; -- "Amazing! ... Come, let us push on."
SAY_ARE_YOU_HURT   = 369; -- "My thanks. Tell me, are you hurt?"
SAY_ATTACKED_AGAIN = 370; -- "The gods are not with us this day..."
SAY_IM_HURT        = 371; -- "I-I'm hurt. My wounds are...grievous, I fear."
SAY_RETREAT        = 372; -- "...let us beat the retreat... The camp will wait for us."
SAY_SO_CLOSE       = 374; -- "We are close. So very close..."
SAY_JUST_AHEAD     = 375; -- "There! It is just ahead!"

-- Per-player escort state, keyed by actor id (the VM is process-cached
-- per script path).
escortState = {};

function onCreate(starterPlayer, contentArea, director)
	escortState[starterPlayer.actorId] = {
		done = false,
		startTick = nil,
		lastBarkTick = nil,
		lastRingTick = nil,
		lastNear = 0,
		reminded20 = false,
		reminded10 = false,
		reminded5 = false,
		sawEscortee = false,
		calledNearGoal = false,
		saidSoClose = false,
		mobsLive = {},
		mobsEngaged = {},
		ringActorId = nil,
		flhaminnId = nil,
	};

	-- Zone-170 spawns (seed/096): 43 = F'lhaminn, 44-51 = chinchillas
	-- spaced along the gate→camp route.
	local flhaminn = GetWorldManager().SpawnBattleNpcById(43, contentArea);
	escortState[starterPlayer.actorId].flhaminnId = flhaminn.actorId;
	local mobs = {};
	for bnpcId = 44, 51 do
		table.insert(mobs, GetWorldManager().SpawnBattleNpcById(bnpcId, contentArea));
	end

	-- Minimap duty halo riding F'lhaminn.
	local ring = contentArea:SpawnActor(ESCORT_RING_CLASS, "escortAreaRange", WARP_IN.x, WARP_IN.y, WARP_IN.z, 0);
	escortState[starterPlayer.actorId].ringActorId = ring.actorId;

	-- F'lhaminn is a civilian who never fights (retail: only the
	-- player attacks); civilian idle avoids the weaponless-model
	-- T-pose (the man0g1 kids finding). The chinchillas render
	-- hostile.
	flhaminn:ChangeState(0);
	for i = 1, #mobs do
		mobs[i]:ChangeState(2);
	end

	-- The player keeps the tutorial-style 1-HP floor. F'lhaminn takes
	-- real damage — her wounding is the retail fail condition
	-- ("I-I'm hurt... let us beat the retreat").
	starterPlayer:SetMod(modifiersGlobal.MinimumHpLock, 1);

	director:AddMember(starterPlayer);
	director:AddMember(director);
	director:AddMember(flhaminn);
	director:AddMember(ring);
	for i = 1, #mobs do
		director:AddMember(mobs[i]);
	end
end

-- Load-gap-SAFE music only; the entry banners live in onUpdate's
-- first-sighting latch.
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

local function sayFlhaminn(owner, sayId)
	local quest = owner:GetQuest("Man0u1");
	owner:SendGameMessage(quest, sayId, 0x20);
end

-- FAIL flow (F'lhaminn wounded / 30-minute expiry / confirmed
-- leave-duty): her retreat barks, unbind, teardown, eject to the gate.
-- The sequence is still SEQ_060 — its onStateChange re-arms the gate
-- trigger, so the duty is retryable immediately (retail: "Do not
-- despair. The camp will wait for us.").
local function escortFail(owner, area, state)
	state.done = true;
	sayFlhaminn(owner, SAY_IM_HURT);
	sayFlhaminn(owner, SAY_RETREAT);
	owner:SendGameMessage(GetWorldMaster(), TEXT_UNBOUND_FROM_DUTY, 0x20);
	-- ContentFinished BEFORE the warp-out (tutorial teardown order).
	area:ContentFinished();
	GetWorldManager():WarpToPublicArea(owner, GATE_EJECT.x, GATE_EJECT.y, GATE_EJECT.z, GATE_EJECT.rot);
	-- Drain the director coroutine parked on "man0u1EscortComplete" so
	-- a stale park can't double-fire the arrival flow on a retry.
	sendSignal("man0u1EscortComplete");
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

	-- ---- Timer (the 30-minute ladder, on film for this duty) ----
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

	-- ---- Escortee roster + wounding-fail (rosters are live-only) ----
	local flhaminn = nil;
	for i = 1, #allies do
		local ally = allies[i];
		if ally and ally.actorId == state.flhaminnId then
			flhaminn = ally;
		end
	end
	if (flhaminn ~= nil) then
		if not state.sawEscortee then
			-- First live sighting = the duty is underway. Retail entry
			-- order: protect banner → bound-by-duty → timer → her
			-- "dangers along the way" opener.
			owner:SendGameMessage(GetWorldMaster(), TEXT_PROTECT_ESCORTEE, 0x20, flhaminn.actorId);
			owner:SendGameMessage(GetWorldMaster(), TEXT_BOUND_BY_DUTY, 0x20);
			owner:SendGameMessage(GetWorldMaster(), TEXT_TIME_REMAINING, 0x20, ESCORT_LIMIT_MINUTES);
			sayFlhaminn(owner, SAY_DANGERS_AHEAD);
		end
		state.sawEscortee = true;
	elseif state.sawEscortee then
		-- F'lhaminn fell → retreat-and-retry.
		escortFail(owner, area, state);
		return;
	else
		-- Pre-onCreate tick (roster not populated yet) — wait.
		return;
	end

	-- Skip actors whose roster position is still unsynced (0,0,0) — on
	-- the spawn tick every distance reads ~0 (the man0l1 round-7h
	-- invisible-enemies rule).
	local function positionLive(a)
		return a and not (a.positionX == 0 and a.positionY == 0 and a.positionZ == 0);
	end
	if not positionLive(flhaminn) or not positionLive(owner) then
		return;
	end

	-- ---- Ambushes: a live chinchilla inside ENGAGE_RADIUS of
	-- F'lhaminn or the player pulls onto the PLAYER (retail: only the
	-- player fights). ----
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
				dist2d(mob.positionX, mob.positionZ, flhaminn.positionX, flhaminn.positionZ),
				dist2d(mob.positionX, mob.positionZ, owner.positionX, owner.positionZ))
			if dMob <= HOLD_RADIUS then
				nearLive = nearLive + 1
			end
			if mob:IsEngaged() then
				anyEngaged = true
				if not state.mobsEngaged[mob.actorId] then
					state.mobsEngaged[mob.actorId] = true;
					owner:SendGameMessageLocalizedDisplayName(GetWorldMaster(), TEXT_MOB_ENGAGED, 0x20, CHINCHILLA_DISPLAY_ID);
					sayFlhaminn(owner, SAY_NOT_TOO_STRONG);
				end
			end
			if dMob <= ENGAGE_RADIUS and not mob:IsEngaged() then
				allyGlobal.EngageTarget(mob, owner)
			end
		end
	end
	-- Defeat announcements: a previously-seen chinchilla vanishing from
	-- the live-only roster died this tick.
	for id in pairs(state.mobsLive) do
		if not liveIds[id] then
			owner:SendGameMessageLocalizedDisplayName(GetWorldMaster(), TEXT_MOB_DEFEATED, 0x20, CHINCHILLA_DISPLAY_ID);
		end
	end
	state.mobsLive = liveIds;

	-- Minimap halo follows F'lhaminn EVERY tick (moveState 2 — the
	-- client glides state-0 moves and the halo falls off the minimap).
	if ring ~= nil and (state.lastRingTick == nil or tick - state.lastRingTick >= RING_MOVE_TICKS) then
		state.lastRingTick = tick;
		if RECORD_MODE then
			ring:MoveTo(owner.positionX, owner.positionY, owner.positionZ, 0.0, 2);
		else
			ring:MoveTo(flhaminn.positionX, flhaminn.positionY, flhaminn.positionZ, 0.0, 2);
		end
	end

	-- Wave-outcome beat: contested count dropping back to zero = this
	-- ambush cleared (her celebration rotation is on film).
	if nearLive == 0 and state.lastNear > 0 then
		sayFlhaminn(owner, (tick % 2 == 0) and SAY_WELL_DONE or SAY_ARE_YOU_HURT);
		sayFlhaminn(owner, SAY_AMAZING);
	end
	state.lastNear = nearLive;

	-- ---- Hold while contested ----
	if nearLive > 0 or anyEngaged then
		return
	end

	-- ---- Arrival: the player reaches the camp, F'lhaminn in tow →
	-- duty complete, the director takes the arrival beat. ----
	if dist2d(owner.positionX, owner.positionZ, ARRIVAL_GOAL.x, ARRIVAL_GOAL.z) <= ARRIVAL_RADIUS then
		state.done = true;
		sayFlhaminn(owner, SAY_JUST_AHEAD);
		sendSignal("man0u1EscortComplete");
		return;
	end

	-- ---- F'lhaminn FOLLOWS the player at run pace, at the player's
	-- real ground Y (the sibling player-leads model). ----
	local dPlayer = dist2d(flhaminn.positionX, flhaminn.positionZ, owner.positionX, owner.positionZ);
	if dPlayer > LEAD_DISTANCE then
		local d = math.max(dPlayer, 0.001);
		local dx = (owner.positionX - flhaminn.positionX) / d;
		local dz = (owner.positionZ - flhaminn.positionZ) / d;
		local step = math.min(RUN_STEP, dPlayer);
		flhaminn:MoveTo(flhaminn.positionX + dx * step, owner.positionY, flhaminn.positionZ + dz * step,
			math.atan(dx, dz), 2);
	end

	-- Near-goal call-out as the player closes on the camp.
	if not state.saidSoClose
			and dist2d(owner.positionX, owner.positionZ, ARRIVAL_GOAL.x, ARRIVAL_GOAL.z) <= 60.0 then
		state.saidSoClose = true;
		sayFlhaminn(owner, SAY_SO_CLOSE);
	end
end

-- Leave-duty teardown (the commandContent confirmed-leave): same
-- eject-and-retry flow as a wounding/timeout fail.
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
