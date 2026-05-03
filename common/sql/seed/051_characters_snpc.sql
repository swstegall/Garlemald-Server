-- "Standard NPC" (SNpc / Path Companion) per-player scratchpad —
-- the cinematic-time fellow NPC the player picks during the
-- man200 MSQ branch. C# `Player.SetSNpc(nickname, actorClassId,
-- classType)` writes these four fields; the matching getters
-- (`GetSNpcNickname` etc.) read them back when the cinematic
-- delegateEvent calls in man200 / man206 forward them to the
-- client for fellow-NPC rendering.
--
-- Per meteor-decomp's authoritative engine inventory
-- (`build/wire/cpp_bindings.md`), `playerbaseclass` only exposes
-- `getCutSceneReplaySnpc{Coordinate,Nickname,Personality,Skin}_cpp`
-- — a DIFFERENT code path that reads from the cinematic-replay
-- subsystem, not from the player's persistent SNpc selection.
-- The `GetSNpc*` Lua bindings garlemald inherited from
-- project-meteor are server-side conveniences that read these
-- columns; see `feedback_meteor_decomp_authoritative_for_engine_bindings.md`.

ALTER TABLE characters ADD COLUMN snpc_nickname TEXT NOT NULL DEFAULT '';
ALTER TABLE characters ADD COLUMN snpc_skin INTEGER NOT NULL DEFAULT 0;
ALTER TABLE characters ADD COLUMN snpc_personality INTEGER NOT NULL DEFAULT 0;
ALTER TABLE characters ADD COLUMN snpc_coordinate INTEGER NOT NULL DEFAULT 0;
