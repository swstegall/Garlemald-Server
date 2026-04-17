//! Map-server DB layer. Ported pragmatically from `Database.cs` (2782 lines):
//! only the queries the Phase-4 login-and-spawn path actually needs. The
//! remaining queries (quests, guildleves, search, debt, market) are left for
//! incremental ports — adding one is mechanical and the API pattern is
//! established here.
#![allow(dead_code)]

use anyhow::{Context, Result};
use mysql_async::{Pool, Row, prelude::*};

use crate::data::{ItemData, InventoryItem};

pub struct Database {
    pool: Pool,
}

impl Database {
    pub fn new(url: &str) -> Result<Self> {
        let pool = Pool::from_url(url).context("parsing mysql url")?;
        Ok(Self { pool })
    }

    pub async fn ping(&self) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "SELECT 1".ignore(&mut conn).await?;
        Ok(())
    }

    /// Load the basic character row that the zone-server needs when a session
    /// opens (name, zone, position, appearance).
    pub async fn load_character(&self, chara_id: u32) -> Result<Option<CharacterRow>> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<Row> = r"SELECT
                id, userId, slot, serverId, name,
                currentZoneId, destinationZoneId,
                positionX, positionY, positionZ, rotation,
                currentActiveLinkshell, guardian, birthMonth, birthDay,
                initialTown, tribe
            FROM characters WHERE id = :cid"
            .with(params! { "cid" => chara_id })
            .first(&mut conn)
            .await?;

        Ok(row.map(character_row_from_row))
    }

    /// Bulk-load the full item catalog into memory (matches the C# startup
    /// `Database.LoadItems`).
    pub async fn load_item_catalog(&self) -> Result<Vec<ItemData>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"SELECT id, name, singular, plural, startsWithVowel, kana,
                                   description, icon, rarity, itemUICategory, stackSize,
                                   itemLevel, equipLevel, price, buyPrice, sellPrice,
                                   bazaarCategory,
                                   isExclusive, isRare, isEx, isDyeable, isTradable,
                                   isUntradable, isSoldable
                             FROM server_items"
            .with(())
            .fetch(&mut conn)
            .await?;
        Ok(rows.into_iter().map(item_data_from_row).collect())
    }

    /// Load every inventory row for a character.
    pub async fn load_inventory(&self, chara_id: u32) -> Result<Vec<InventoryItem>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"SELECT uniqueId, itemId, quantity, quality, slot, linkSlot,
                                   itemPackage, durability, useCount, materiaId, materiaLife,
                                   mainQuality, polish, param1, param2, param3, spiritbind
                             FROM characters_inventory WHERE characterId = :cid"
            .with(params! { "cid" => chara_id })
            .fetch(&mut conn)
            .await?;
        Ok(rows.into_iter().map(inventory_item_from_row).collect())
    }

    /// Persist the character's current position / zone on zone change.
    pub async fn save_character_position(
        &self,
        chara_id: u32,
        zone_id: u32,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"UPDATE characters SET currentZoneId = :zid,
                                positionX = :x, positionY = :y, positionZ = :z,
                                rotation = :r
          WHERE id = :cid"
            .with(params! {
                "zid" => zone_id,
                "x" => x, "y" => y, "z" => z, "r" => rotation,
                "cid" => chara_id,
            })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Row extractors. Using by-name access means adding columns is non-breaking.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct CharacterRow {
    pub id: u32,
    pub user_id: u32,
    pub slot: u16,
    pub server_id: u16,
    pub name: String,
    pub current_zone_id: u32,
    pub destination_zone_id: u32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub rotation: f32,
    pub current_active_linkshell: String,
    pub guardian: u8,
    pub birth_month: u8,
    pub birth_day: u8,
    pub initial_town: u8,
    pub tribe: u8,
}

fn character_row_from_row(mut row: Row) -> CharacterRow {
    CharacterRow {
        id: row.take("id").unwrap_or_default(),
        user_id: row.take("userId").unwrap_or_default(),
        slot: row.take("slot").unwrap_or_default(),
        server_id: row.take("serverId").unwrap_or_default(),
        name: row.take("name").unwrap_or_default(),
        current_zone_id: row.take("currentZoneId").unwrap_or_default(),
        destination_zone_id: row.take("destinationZoneId").unwrap_or_default(),
        position_x: row.take("positionX").unwrap_or_default(),
        position_y: row.take("positionY").unwrap_or_default(),
        position_z: row.take("positionZ").unwrap_or_default(),
        rotation: row.take("rotation").unwrap_or_default(),
        current_active_linkshell: row.take("currentActiveLinkshell").unwrap_or_default(),
        guardian: row.take("guardian").unwrap_or_default(),
        birth_month: row.take("birthMonth").unwrap_or_default(),
        birth_day: row.take("birthDay").unwrap_or_default(),
        initial_town: row.take("initialTown").unwrap_or_default(),
        tribe: row.take("tribe").unwrap_or_default(),
    }
}

fn item_data_from_row(mut row: Row) -> ItemData {
    ItemData {
        id: row.take("id").unwrap_or_default(),
        name: row.take("name").unwrap_or_default(),
        singular: row.take("singular").unwrap_or_default(),
        plural: row.take("plural").unwrap_or_default(),
        start_with_vowel: row.take::<i64, _>("startsWithVowel").unwrap_or(0) != 0,
        kana: row.take("kana").unwrap_or_default(),
        description: row.take("description").unwrap_or_default(),
        icon: row.take("icon").unwrap_or_default(),
        rarity: row.take("rarity").unwrap_or_default(),
        item_ui_category: row.take("itemUICategory").unwrap_or_default(),
        stack_size: row.take("stackSize").unwrap_or_default(),
        item_level: row.take("itemLevel").unwrap_or_default(),
        equip_level: row.take("equipLevel").unwrap_or_default(),
        price: row.take("price").unwrap_or_default(),
        buy_price: row.take("buyPrice").unwrap_or_default(),
        sell_price: row.take("sellPrice").unwrap_or_default(),
        bazaar_category: row.take("bazaarCategory").unwrap_or_default(),
        is_exclusive: row.take::<i64, _>("isExclusive").unwrap_or(0) != 0,
        is_rare: row.take::<i64, _>("isRare").unwrap_or(0) != 0,
        is_ex: row.take::<i64, _>("isEx").unwrap_or(0) != 0,
        is_dyeable: row.take::<i64, _>("isDyeable").unwrap_or(0) != 0,
        is_tradable: row.take::<i64, _>("isTradable").unwrap_or(0) != 0,
        is_untradable: row.take::<i64, _>("isUntradable").unwrap_or(0) != 0,
        is_soldable: row.take::<i64, _>("isSoldable").unwrap_or(0) != 0,
        unknown1: 0,
        unknown2: 0,
    }
}

fn inventory_item_from_row(mut row: Row) -> InventoryItem {
    let mut item = InventoryItem {
        unique_id: row.take("uniqueId").unwrap_or_default(),
        item_id: row.take("itemId").unwrap_or_default(),
        quantity: row.take("quantity").unwrap_or(1),
        quality: row.take("quality").unwrap_or(1),
        slot: row.take("slot").unwrap_or(0xFFFF),
        link_slot: row.take("linkSlot").unwrap_or(0xFFFF),
        item_package: row.take("itemPackage").unwrap_or(0xFFFF),
        tag: Default::default(),
    };
    item.tag.durability = row.take("durability").unwrap_or_default();
    item.tag.use_count = row.take("useCount").unwrap_or_default();
    item.tag.materia_id = row.take("materiaId").unwrap_or_default();
    item.tag.materia_life = row.take("materiaLife").unwrap_or_default();
    item.tag.main_quality = row.take("mainQuality").unwrap_or_default();
    item.tag.polish = row.take("polish").unwrap_or_default();
    item.tag.param1 = row.take("param1").unwrap_or_default();
    item.tag.param2 = row.take("param2").unwrap_or_default();
    item.tag.param3 = row.take("param3").unwrap_or_default();
    item.tag.spiritbind = row.take("spiritbind").unwrap_or_default();
    item
}
