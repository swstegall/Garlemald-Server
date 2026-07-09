require ("global")
require ("modifiers")
-- onUpdate calls allyGlobal.EngageTarget; without this require the
-- global is nil and every onUpdate tick errors (swallowed by the
-- ticker drain). Same shape as SimpleContent30010/30002.
require ("ally")

-- Ul'dah SEQ_005 Sapphire Avenue fight (Man0u0 "Flowers for All") —
-- port of the SimpleContent30010 rewrite to the goobbue fight
-- (Garlemald-Server #26, sibling of the #25 Limsa port). The upstream
-- import spawned Niellefresne/Thancred/the goobbue as synthetic
-- contentArea:SpawnActor props: no combat AI, no HP model, no kill
-- gate. BNpc seed rows live in
-- common/sql/seed/055_uldah_seq005_tutorial.sql (spawn ids 13-15).

function onCreate(starterPlayer, contentArea, director)
	-- Reset the engagement latch: the engine caches ONE VM per script
	-- path for the whole server process, so the top-level
	-- `battleStarted = false` below runs only once — a second run of
	-- this tutorial (new character, or re-entry after a disconnect)
	-- would otherwise inherit battleStarted=true and let the allies
	-- attack before the targeting tutorial returns.
	battleStarted = false;
	-- Clear this player's kill-EXP counter entry (see onUpdate) — a
	-- stale entry from an aborted previous run would mis-pay the grant.
	-- Other players' in-flight entries are left alone.
	tutorialLiveHostiles = tutorialLiveHostiles or {};
	tutorialLiveHostiles[starterPlayer.actorId] = nil;
	thancred = GetWorldManager().SpawnBattleNpcById(14, contentArea);
	niellefresne = GetWorldManager().SpawnBattleNpcById(15, contentArea);
	mob1 = GetWorldManager().SpawnBattleNpcById(13, contentArea);
	-- Active/engaged MainState for the melee ally + the goobbue so they
	-- stand up and render hostile (SimpleContent30010 pattern).
	-- Niellefresne (healer/caster) stays at the default passive state
	-- like Papalymo does in the Gridania fight.
	thancred:ChangeState(2);
	mob1:ChangeState(2);
	-- Party-add the allies so the HUD roster shows their HP bars (the
	-- 1.x party list reads the real party group, not the content
	-- group). ContentFinished clears the transient roster at teardown.
	starterPlayer.currentParty:AddMember(niellefresne.actorId);
	starterPlayer.currentParty:AddMember(thancred.actorId);
	-- No-die guarantees for the tutorial (MinimumHpLock floor-1 clamp);
	-- the player's lock is cleared at ContentFinished.
	starterPlayer:SetMod(modifiersGlobal.MinimumHpLock, 1);
	thancred:SetMod(modifiersGlobal.MinimumHpLock, 1);
	niellefresne:SetMod(modifiersGlobal.MinimumHpLock, 1);

	-- NO arena fence prop. The upstream import spawned 1090385 here
	-- ("openingstoper") and its armed push circles outlived the
	-- teardown's RemoveActor on the 1.23b client: the orphan "exit"
	-- circle fired against the ghost after the SEQ_010 warp (client
	-- crash ~4s into zone 175), and the disarm-before-remove attempt
	-- crashed the client even earlier (see apply_despawn_actor's
	-- parity note). Limsa's SimpleContent30002 — the upstream-tested
	-- twin — ships no stopper either; the engagement latch plus the
	-- tiny arena already contain the fight. (#26, PR #156 Rounds 6-7.)

	director:AddMember(starterPlayer);
	director:AddMember(director);
	director:AddMember(niellefresne);
	director:AddMember(thancred);
	director:AddMember(mob1);
end

function onDestroy()

end

-- Everyone fights once the player commits (#28 S2.5). Script-VM
-- global: persists across onUpdate calls. The VM is process-cached
-- per script path (NOT per content area), so onCreate re-arms it for
-- each fresh run.
battleStarted = false;
-- Per-PLAYER live-hostile counts from the previous tick — drives the
-- kill-EXP grant below. Keyed by player actor id because this VM (and
-- so this table) is shared by EVERY session ticking this script; a
-- plain scalar would interleave concurrent same-city runs and mint
-- phantom EXP. onCreate clears the starting player's entry.
tutorialLiveHostiles = {};

function onUpdate(tick, area)
	if not area then return end
	local players = area:GetPlayers()
	local mobs    = area:GetMonsters()   -- live-only (dead filtered by S0.5)
	local allies  = area:GetAllies()

	local engagedPlayer, firstPlayer = nil, nil
	for player in players do
		if player then
			if not firstPlayer then firstPlayer = player end
			if player:IsEngaged() and player.target then
				engagedPlayer = player
				break
			end
		end
	end

	-- Tutorial kill EXP: retail granted 3000 EXP for downing the goobbue
	-- (single hostile, vs Limsa/Gridania's 1000-per-mob trio) — enough to
	-- clear level 2 (570 SP) in one grant. (FFXIVenturer "Flowers for
	-- All" guide: "You will receive 3000 EXP and a Velodyna Cosmos
	-- (quest item) for completing the sequence".) `mobs` is live-only, so
	-- a drop in the count since the last tick = the kill. Granted to the
	-- player regardless of who landed the blow — retail pays the full
	-- amount on ally killing blows too, which the onKillBNpc quest hook's
	-- player-only attacker gate would miss. The `#allies > 0` gate is
	-- the "fight is set up" signal: pre-onCreate ticks (no roster yet)
	-- never touch the counter, so a stale count can't pay out against
	-- an unspawned fight — and the allies are MinimumHpLock-floored, so
	-- the roster stays populated through the kill's grant.
	if #allies > 0 then
		local owner = engagedPlayer or firstPlayer
		if owner then
			local prev = tutorialLiveHostiles[owner.actorId]
			if prev ~= nil and #mobs < prev then
				for _ = 1, prev - #mobs do
					owner:AddExp(3000, owner.charaWork.parameterSave.state_mainSkill[0], 0)
				end
			end
			tutorialLiveHostiles[owner.actorId] = #mobs
		end
	end

	if not battleStarted then
		-- Nothing moves until the player attacks. Latching on the
		-- player's engagement keeps the goobbue alive and targetable
		-- through processTtrBtl002's targeting tutorial (allies killing
		-- it early could soft-lock _waitForTargetTutorial client-side).
		if not engagedPlayer then return end
		battleStarted = true
	end

	-- Allies: spread across live mobs; re-engage as mobs die.
	local mi = 0
	for i = 1, #allies do
		local ally = allies[i]
		if ally and not ally:IsEngaged() and #mobs > 0 then
			local target = mobs[(mi % #mobs) + 1]
			mi = mi + 1
			ally:SetMod(modifiersGlobal.MovementSpeed, 8)
			allyGlobal.EngageTarget(ally, target)   -- Engage + AddBaseHate (ally.lua)
		end
	end
	-- The goobbue: proactive once battle starts (retaliation hate
	-- already covers it when struck; this brings it in if the allies
	-- opened first).
	for i = 1, #mobs do
		local mob = mobs[i]
		if mob and not mob:IsEngaged() then
			local foe = engagedPlayer or (#allies > 0 and allies[1])
			if foe then allyGlobal.EngageTarget(mob, foe) end
		end
	end
end
