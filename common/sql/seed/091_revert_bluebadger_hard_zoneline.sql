-- Revert the Blue Badger Gate hard-zoneline experiment (Garlemald-Server #41).
--
-- The prior seed (090, now deleted) modelled the New Gridania 155 -> Central
-- Shroud 150 crossing as a hard loading-screen zoneline (a full DoZoneChange to
-- a fixed arrival point). Per playtest that hard teleport was not accurate to
-- the game, so the zoneline mechanism (server_zonelines table + its Rust) has
-- been removed. This seed reverses seed/090's data effects for databases that
-- already applied it:
--   1. Drop the now-unused server_zonelines table.
--   2. Restore the 155<->150 seamless boundary row that 090 deleted (the values
--      seed/089 last set for the Blue Badger Gate boxes).
--
-- Idempotent: safe whether or not 090 ever ran. A fresh DB never created the
-- table (DROP ... IF EXISTS is a no-op) and still carries the row from
-- seed/029+089; a migrated DB has the table (dropped) and lost the row
-- (re-inserted). The DELETE+INSERT below normalises either starting state to
-- exactly one row.

DROP TABLE IF EXISTS server_zonelines;

DELETE FROM "server_seamless_zonechange_bounds"
    WHERE "regionId" = 103 AND "zoneId1" = 155 AND "zoneId2" = 150;

INSERT INTO "server_seamless_zonechange_bounds"
    ("regionId", "zoneId1", "zoneId2",
     "zone1_boundingbox_x1", "zone1_boundingbox_y1", "zone1_boundingbox_x2", "zone1_boundingbox_y2",
     "zone2_boundingbox_x1", "zone2_boundingbox_y1", "zone2_boundingbox_x2", "zone2_boundingbox_y2",
     "merge_boundingbox_x1", "merge_boundingbox_y1", "merge_boundingbox_x2", "merge_boundingbox_y2")
VALUES
    (103, 155, 150,
     140, -1168, 181, -1138,
     182, -1168, 220, -1138,
     181, -1168, 182, -1138);
