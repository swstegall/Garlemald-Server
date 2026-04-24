-- Per-retainer bazaar inventory.
--
-- Retainers stock three item packages in the retail 1.x layout —
-- NORMAL (player-facing trade), CURRENCY_CRYSTALS, and BAZAAR (the
-- "for sale with a price tag" bag the bazaar-check flow reads).
-- This table is scoped to the BAZAAR bag specifically because
-- bazaar listings carry a per-item gil price the other two bags
-- don't need, and the BAZAAR bag is the only retainer-exclusive
-- package (NORMAL + CURRENCY overlap conceptually with player
-- inventory layouts, so they share the `characters_inventory`
-- keying under the same owner-scoping rules).
--
-- Columns mirror `characters_inventory`'s `(characterId,
-- serverItemId, itemPackage, slot)` shape but keyed on
-- `retainerId` (the `server_retainers.id`, not the composite
-- actor id) so ownership survives retainer-despawn and player
-- logout. `priceGil` is the per-item-unit price the owner listed
-- the stack at; buyers pay `priceGil * quantity` in the BazaarDeal
-- flow once that packet family lands.
--
-- `createdUtc` is the UNIX-epoch second the listing was added —
-- retail's bazaar UI displays "listed X hours ago", and a future
-- cleanup pass can reap stale listings by age. `updatedUtc`
-- refreshes whenever the price changes.

CREATE TABLE IF NOT EXISTS characters_retainer_bazaar (
    retainerId   INTEGER NOT NULL,
    serverItemId INTEGER NOT NULL,
    slot         INTEGER NOT NULL,
    priceGil     INTEGER NOT NULL DEFAULT 0,
    createdUtc   INTEGER NOT NULL DEFAULT 0,
    updatedUtc   INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (retainerId, serverItemId)
);

CREATE INDEX IF NOT EXISTS idx_retainer_bazaar_retainer
    ON characters_retainer_bazaar (retainerId);
