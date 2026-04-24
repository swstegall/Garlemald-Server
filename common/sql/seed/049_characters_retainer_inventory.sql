-- Retainer personal-inventory table. Tier 4 #14 C — the "retainer
-- holds items for the player" side of retainer storage, separate
-- from the bazaar listings which live in `characters_retainer_bazaar`.
--
-- Keyed by `retainerId` rather than `characterId` so retainer
-- storage stays logically disjoint from the player's own inventory.
-- Matches the shape of `characters_retainer_bazaar` so the two
-- tables can be queried side-by-side when rendering the retainer's
-- full holdings.

DROP TABLE IF EXISTS "characters_retainer_inventory";
CREATE TABLE IF NOT EXISTS "characters_retainer_inventory" (
    "retainerId"   INTEGER NOT NULL,
    "serverItemId" INTEGER NOT NULL,
    "itemPackage"  INTEGER NOT NULL DEFAULT 0,
    "slot"         INTEGER NOT NULL,
    PRIMARY KEY ("retainerId", "serverItemId")
);

CREATE INDEX IF NOT EXISTS idx_retainer_inventory_retainer
    ON "characters_retainer_inventory" ("retainerId");
