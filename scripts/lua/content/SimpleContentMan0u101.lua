require ("global")
require ("modifiers")
-- onUpdate drives engagement through allyGlobal.EngageTarget; without
-- this require the global is nil and every tick errors (swallowed by
-- the ticker drain). Same shape as the sibling content scripts.
require ("ally")
-- SEQ_* constants (proven loadable outside the quest runtime — the
-- sibling directors/content require their quest scripts the same way).
require ("quests/man/man0u1")

-- Man0u1 SEQ_015 — the Coliseum first-round match (Garlemald-Server
-- #53). Retail (both full playthroughs + the beta uploader's notes):
-- a 1v1 on the bloodsands vs the TOURNEY GLADIATOR — an immortal
-- humanoid (Light Stab / Heavy Stab / Red Lotus / Light Slash on
-- film) — the player LOSES by KO or timeout ("Otto/Siddeon is
-- defeated."), and the loss IS the quest's completion condition
-- (Mirke Loremonger footnote: "There is no way to win.").
--
-- Port: the gladiator carries MinimumHpLock (unkillable, retail), and
-- the PLAYER does too (garlemald's death/return flow isn't wired for
-- content — the opener-tutorial precedent). "Defeat" is detected as
-- the player pinned at the 1-HP floor (or the round timer expiring),
-- which emits the retail defeat line and hands the completion beat to
-- QuestDirectorMan0u101 (post-match cutscene + teardown).

TEXT_BOUND_BY_DUTY   = 50011; -- "You are now bound by duty."

TICKS_PER_SECOND = 2;
-- No timer line was OCR'd in either retail arena capture; five
-- minutes is a generous stand-in for the "timing out" loss path the
-- beta uploader describes.
ROUND_LIMIT_TICKS = 5 * 60 * TICKS_PER_SECOND;
ARENA_DUTY_MUSIC = 37; -- the 1.x duty theme (sibling escorts)

-- Per-player fight state (the VM is process-cached per script path).
arenaState = {};

function onCreate(starterPlayer, contentArea, director)
	arenaState[starterPlayer.actorId] = {
		done = false,
		startTick = nil,
		bannerShown = false,
		gladiatorId = nil,
	};

	-- The Tourney Gladiator (class 2280157, display 3280156 —
	-- bahamut gamedata; seed/096 spawn row).
	local gladiator = GetWorldManager().SpawnBattleNpcById(42, contentArea);
	arenaState[starterPlayer.actorId].gladiatorId = gladiator.actorId;

	-- Hostile battle stance; immortal (retail: "the gladiator is
	-- immortal, you will lose").
	gladiator:ChangeState(2);
	gladiator:SetMod(modifiersGlobal.MinimumHpLock, 1);

	-- The player's no-die floor — pinned there = the loss (see
	-- onUpdate).
	starterPlayer:SetMod(modifiersGlobal.MinimumHpLock, 1);

	director:AddMember(starterPlayer);
	director:AddMember(director);
	director:AddMember(gladiator);
end

-- Load-gap-SAFE music only; the banners live in onUpdate's
-- first-sighting latch (0x0157 messages shipped into the Now-Loading
-- gap crash the client).
function onZoneIn(player, contentArea, director)
	player:ChangeMusic(ARENA_DUTY_MUSIC);
end

function onDestroy()
end

function onUpdate(tick, area)
	if not area then return end
	local players = area:GetPlayers()
	local mobs    = area:GetMonsters()

	local owner = nil
	for player in players do
		if player then owner = owner or player end
	end
	if not owner then return end
	local state = arenaState[owner.actorId]
	if not state or state.done then return end

	local gladiator = nil
	for i = 1, #mobs do
		local mob = mobs[i]
		if mob and mob.actorId == state.gladiatorId then
			gladiator = mob
		end
	end
	if not gladiator then return end

	-- First-sighting latch: the duty banner, then the gladiator opens
	-- on the player (retail: he engages immediately — the match starts
	-- the moment the sands load).
	if not state.bannerShown then
		state.bannerShown = true;
		state.startTick = tick;
		owner:SendGameMessage(GetWorldMaster(), TEXT_BOUND_BY_DUTY, 0x20);
	end
	if not gladiator:IsEngaged() then
		allyGlobal.EngageTarget(gladiator, owner);
	end

	-- The loss: the player pinned at the MinimumHpLock floor, or the
	-- round expiring. Either way the retail defeat line fires and the
	-- director takes over (post-match cutscene + teardown) — losing IS
	-- completing.
	local elapsed = tick - (state.startTick or tick);
	if (owner:GetHP() <= 1 or elapsed >= ROUND_LIMIT_TICKS) then
		state.done = true;
		-- Pacify the gladiator IN the completion tick: the sibling
		-- escorts structurally forbid a cutscene landing in the same
		-- drain as live combat packets (the arrival gate is
		-- nearLive==0 + not anyEngaged; the director's wait(2)
		-- render-settle exists for exactly this). The arena's loss
		-- happens mid-combat by definition, so the disengage must be
		-- explicit — civilian MainState stops the swings before the
		-- director's kick + processEvent035 go out. (Branch review
		-- finding, 2026-07-09.)
		gladiator:ChangeState(0);
		-- Retail revives the loser for the post-match scene ("Otto is
		-- defeated." then straight to Greinfarr, no weakness). Restore
		-- before the teardown clears the 1-HP pin — a 1-HP exit would
		-- persist into open-world zone 170 later in the quest.
		owner:SetHP(owner:GetMaxHP());
		-- Defeat line. Retail renders "<Name> is defeated." through
		-- the client's own KO machinery, which this pinned loss never
		-- triggers; 30121 with a runtime actor id renders a BLANK
		-- subject (the #199 DispId-sender finding), and player names
		-- have no static sheet id — so the literal-fallback shape
		-- (the sibling escorts' undecoded-bark recipe) carries the
		-- beat until a live probe finds the right wire family.
		owner:SendMessage(0x20, "", "You are defeated.");
		sendSignal("coliseumFought");
		return;
	end
end

-- Leave-duty teardown: retail offers no abandon mid-match, but the
-- command surface can still fire it — treat it as the loss (the fight
-- cannot be failed harder than losing).
function onAbort(player, contentArea, director)
	local state = arenaState[player.actorId];
	if state == nil or state.done then
		return;
	end
	state.done = true;
	sendSignal("coliseumFought");
end
