-- Issue #11: the world/server list moved from the `servers` table to
-- `configs/servers.toml` (see `common::server_list`). Nothing reads the
-- table anymore — lobby builds the character-select list from the TOML and
-- world-server resolves its name the same way. Drop it from existing
-- databases; fresh databases never create it (removed from schema.sql, and
-- the old 036_servers seed was retired).
DROP TABLE IF EXISTS servers;
