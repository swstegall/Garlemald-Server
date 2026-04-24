// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Aetheryte teleport-destination table.
//!
//! Server-side mirror of `scripts/lua/aetheryte.lua::aetheryteTeleportPositions`.
//! Each entry maps an aetheryte id (1280001..=1280125) to the zone +
//! coords the player lands at when they teleport there. Currently only
//! used by `runtime::dispatcher::apply_home_point_revive` — `TeleportCommand.lua`
//! still does its own lookup against the Lua copy because the
//! menu-driven teleport flow runs entirely script-side.
//!
//! Keep this table sorted ascending by aetheryte id; [`lookup`] uses
//! binary search and a debug-build assertion enforces monotonicity.

#![allow(dead_code)]

#[derive(Debug, Clone, Copy)]
pub struct AetheryteSpawn {
    pub aetheryte_id: u32,
    pub zone_id: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Sorted ascending by `aetheryte_id`. Must stay sorted — see
/// [`debug_assert_sorted`] (called from the test below).
pub static AETHERYTE_SPAWNS: &[AetheryteSpawn] = &[
    // ---- La Noscea ----
    AetheryteSpawn { aetheryte_id: 1_280_001, zone_id: 230, x: -407.0,    y:  42.5,    z:  337.0 },     // Limsa Lominsa CAP
    AetheryteSpawn { aetheryte_id: 1_280_002, zone_id: 128, x:   29.97,   y:  45.83,   z:  -35.47 },    // CAP
    AetheryteSpawn { aetheryte_id: 1_280_003, zone_id: 129, x: -991.88,   y:  61.71,   z: -1120.79 },   // CAP
    AetheryteSpawn { aetheryte_id: 1_280_004, zone_id: 129, x: -1883.47,  y:  53.77,   z: -1372.68 },   // CAP
    AetheryteSpawn { aetheryte_id: 1_280_005, zone_id: 130, x:  1123.29,  y:  45.7,    z:  -928.69 },   // CAP
    AetheryteSpawn { aetheryte_id: 1_280_006, zone_id: 135, x: -278.181,  y:  77.63,   z: -2260.79 },   // CAP
    AetheryteSpawn { aetheryte_id: 1_280_007, zone_id: 128, x:  582.47,   y:  54.52,   z:    -1.2 },
    AetheryteSpawn { aetheryte_id: 1_280_008, zone_id: 128, x:  962.836,  y:  46.507,  z:   832.206 },  // Widow Cliffs
    AetheryteSpawn { aetheryte_id: 1_280_009, zone_id: 128, x:  318.0,    y:  24.5,    z:   581.0 },    // Moraby Bay
    AetheryteSpawn { aetheryte_id: 1_280_010, zone_id: 129, x: -636.0,    y:  48.8,    z: -1287.0 },    // Woad Whisper Canyon
    AetheryteSpawn { aetheryte_id: 1_280_011, zone_id: 129, x: -2016.72,  y:  60.055,  z:  -766.962 },  // Isles of Umbra
    AetheryteSpawn { aetheryte_id: 1_280_012, zone_id: 130, x:  1628.0,   y:  60.3,    z:  -449.0 },    // Tiger Helm Island
    AetheryteSpawn { aetheryte_id: 1_280_013, zone_id: 130, x:  1522.0,   y:   1.7,    z:  -669.0 },    // Bloodshore
    AetheryteSpawn { aetheryte_id: 1_280_014, zone_id: 130, x:  1410.0,   y:  53.3,    z: -1650.0 },    // Agelyss Wise
    AetheryteSpawn { aetheryte_id: 1_280_015, zone_id: 135, x: -123.315,  y:  60.061,  z: -1438.8 },    // Zelma's Run
    AetheryteSpawn { aetheryte_id: 1_280_016, zone_id: 135, x: -320.322,  y:  52.835,  z: -1823.68 },   // Bronze Lake
    AetheryteSpawn { aetheryte_id: 1_280_017, zone_id: 135, x: -894.0,    y:  41.2,    z: -2188.0 },    // Oakwood
    AetheryteSpawn { aetheryte_id: 1_280_018, zone_id: 131, x: -1694.5,   y: -19.9,    z: -1534.0 },    // Mistbeard Cove
    AetheryteSpawn { aetheryte_id: 1_280_020, zone_id: 132, x:  1343.5,   y: -54.38,   z:  -870.84 },   // CAP
    // ---- Thanalan ----
    AetheryteSpawn { aetheryte_id: 1_280_031, zone_id: 175, x: -235.0,    y: 185.0,    z:    -3.9 },    // Ul'dah CAP
    AetheryteSpawn { aetheryte_id: 1_280_032, zone_id: 170, x:   33.0,    y: 200.1,    z:  -482.0 },    // Camp Black Brush
    AetheryteSpawn { aetheryte_id: 1_280_033, zone_id: 171, x:  1250.9,   y: 264.0,    z:  -544.2 },    // CAP
    AetheryteSpawn { aetheryte_id: 1_280_034, zone_id: 172, x: -1313.91,  y:  56.023,  z:  -145.597 },  // Camp Horizon
    AetheryteSpawn { aetheryte_id: 1_280_035, zone_id: 173, x: -165.816,  y: 280.002,  z: -1698.45 },   // Camp Bluefog
    AetheryteSpawn { aetheryte_id: 1_280_036, zone_id: 174, x:  1687.64,  y: 296.002,  z:   992.283 },  // Camp Brokenwater
    AetheryteSpawn { aetheryte_id: 1_280_037, zone_id: 170, x:  639.0,    y: 183.9,    z:   122.0 },    // Cactus Basin
    AetheryteSpawn { aetheryte_id: 1_280_038, zone_id: 170, x:  539.0,    y: 215.8,    z:   -14.0 },    // Four Sisters
    AetheryteSpawn { aetheryte_id: 1_280_039, zone_id: 171, x:  1599.0,   y: 256.7,    z:  -233.0 },    // Halatali
    AetheryteSpawn { aetheryte_id: 1_280_040, zone_id: 171, x:  2010.0,   y: 280.3,    z:  -768.0 },    // Burning Wall
    AetheryteSpawn { aetheryte_id: 1_280_041, zone_id: 171, x:  2015.0,   y: 247.8,    z:    64.0 },    // Sandgate
    AetheryteSpawn { aetheryte_id: 1_280_042, zone_id: 172, x: -864.991,  y:  88.84,   z:   375.18 },   // Nophica's Wells
    AetheryteSpawn { aetheryte_id: 1_280_043, zone_id: 172, x: -1653.0,   y:  24.5,    z:  -469.0 },    // Footfalls
    AetheryteSpawn { aetheryte_id: 1_280_044, zone_id: 172, x: -1220.38,  y:  69.854,  z:   194.365 },  // Scorpion Keep
    AetheryteSpawn { aetheryte_id: 1_280_045, zone_id: 173, x: -635.0,    y: 280.0,    z: -1797.0 },    // Hidden Gorge
    AetheryteSpawn { aetheryte_id: 1_280_046, zone_id: 173, x:  447.0,    y: 259.1,    z: -2158.0 },    // Sea of Spires
    AetheryteSpawn { aetheryte_id: 1_280_047, zone_id: 173, x: -710.0,    y: 280.4,    z: -2212.0 },    // Cutters Pass
    AetheryteSpawn { aetheryte_id: 1_280_048, zone_id: 174, x:  1797.0,   y: 248.0,    z:  1856.0 },    // Red Labyrinth
    AetheryteSpawn { aetheryte_id: 1_280_049, zone_id: 174, x:  1185.0,   y: 279.8,    z:  1407.0 },    // Burnt Lizard Creek
    AetheryteSpawn { aetheryte_id: 1_280_050, zone_id: 174, x:  2416.0,   y: 248.3,    z:  1535.0 },    // Zanr'ak
    AetheryteSpawn { aetheryte_id: 1_280_052, zone_id: 176, x:   80.056,  y: 167.929,  z: -1267.94 },   // Nanawa Mines
    AetheryteSpawn { aetheryte_id: 1_280_054, zone_id: 178, x: -620.374,  y: 110.429,  z:  -113.903 },  // Copperbell Mines
    // ---- Black Shroud ----
    AetheryteSpawn { aetheryte_id: 1_280_061, zone_id: 206, x: -120.0,    y:  16.0,    z: -1332.0 },    // Gridania CAP
    AetheryteSpawn { aetheryte_id: 1_280_062, zone_id: 150, x:  288.0,    y:   4.0,    z:  -543.928 },  // CAP
    AetheryteSpawn { aetheryte_id: 1_280_063, zone_id: 151, x:  1702.0,   y:  20.0,    z:  -862.0 },    // CAP
    AetheryteSpawn { aetheryte_id: 1_280_064, zone_id: 152, x: -1052.0,   y:  20.0,    z: -1760.0 },    // CAP
    AetheryteSpawn { aetheryte_id: 1_280_065, zone_id: 153, x: -1566.035, y: -11.89,   z:  -550.51 },   // CAP
    AetheryteSpawn { aetheryte_id: 1_280_066, zone_id: 154, x:  734.0,    y: -12.0,    z:  1126.0 },    // CAP
    AetheryteSpawn { aetheryte_id: 1_280_067, zone_id: 150, x:  -94.07,   y:   4.0,    z:  -543.16 },   // Humblehearth
    AetheryteSpawn { aetheryte_id: 1_280_068, zone_id: 150, x: -285.0,    y: -21.8,    z:   -46.0 },    // Sorrel Haven
    AetheryteSpawn { aetheryte_id: 1_280_069, zone_id: 150, x:  636.0,    y:  16.2,    z:  -324.0 },    // Five Hangs
    AetheryteSpawn { aetheryte_id: 1_280_070, zone_id: 151, x:  1529.83,  y:  26.991,  z: -1140.15 },   // Verdant Drop
    AetheryteSpawn { aetheryte_id: 1_280_071, zone_id: 151, x:  1296.0,   y:  47.2,    z: -1534.0 },    // Lynxpelt Patch
    AetheryteSpawn { aetheryte_id: 1_280_072, zone_id: 151, x:  2297.02,  y:  31.546,  z:  -697.828 },  // Larkscall
    AetheryteSpawn { aetheryte_id: 1_280_073, zone_id: 152, x: -883.769,  y:  34.688,  z: -2187.45 },   // Treespeak
    AetheryteSpawn { aetheryte_id: 1_280_074, zone_id: 152, x: -1567.0,   y:  16.1,    z: -2593.0 },    // Aldersprings
    AetheryteSpawn { aetheryte_id: 1_280_075, zone_id: 152, x: -800.277,  y:  32.0,    z: -2785.4 },    // Lasthold
    AetheryteSpawn { aetheryte_id: 1_280_076, zone_id: 153, x: -1908.0,   y:   0.3,    z: -1042.0 },    // Lichenweed
    AetheryteSpawn { aetheryte_id: 1_280_077, zone_id: 153, x: -2158.0,   y: -46.1,    z:  -166.0 },    // Murmur Rills
    AetheryteSpawn { aetheryte_id: 1_280_078, zone_id: 153, x: -1333.0,   y: -14.2,    z:   324.0 },    // Turning Leaf
    AetheryteSpawn { aetheryte_id: 1_280_079, zone_id: 154, x:  991.0,    y: -11.8,    z:   600.0 },    // Silent Arbor
    AetheryteSpawn { aetheryte_id: 1_280_080, zone_id: 154, x:  1126.0,   y:  -0.1,    z:  1440.0 },    // Longroot
    AetheryteSpawn { aetheryte_id: 1_280_081, zone_id: 154, x:  189.0,    y:   0.1,    z:  1337.0 },    // Snakemolt
    AetheryteSpawn { aetheryte_id: 1_280_082, zone_id: 157, x: -687.916,  y: -15.308,  z: -2063.94 },   // Mun-Tuy Cellars
    AetheryteSpawn { aetheryte_id: 1_280_083, zone_id: 158, x:  314.801,  y: -36.2,    z:  -167.843 },  // Tam-Tara Deepcroft
    // ---- Coerthas ----
    AetheryteSpawn { aetheryte_id: 1_280_092, zone_id: 143, x:  216.0,    y: 302.1,    z:  -258.0 },    // Camp Dragonhead
    AetheryteSpawn { aetheryte_id: 1_280_093, zone_id: 144, x:  1122.21,  y: 270.004,  z: -1149.29 },   // Camp Crooked Fork
    AetheryteSpawn { aetheryte_id: 1_280_094, zone_id: 145, x:  1500.78,  y: 206.036,  z:   767.546 },  // Camp Glory
    AetheryteSpawn { aetheryte_id: 1_280_095, zone_id: 147, x: -159.828,  y: 222.037,  z:  1154.81 },   // Camp Ever Lakes
    AetheryteSpawn { aetheryte_id: 1_280_096, zone_id: 148, x: -1760.36,  y: 270.059,  z:  -194.713 },  // Camp Riversmeet
    AetheryteSpawn { aetheryte_id: 1_280_097, zone_id: 143, x: -517.0,    y: 207.9,    z:   543.0 },    // Boulder Downs
    AetheryteSpawn { aetheryte_id: 1_280_098, zone_id: 143, x:  190.0,    y: 367.4,    z:  -662.0 },    // Prominence Point
    AetheryteSpawn { aetheryte_id: 1_280_099, zone_id: 143, x:  960.0,    y: 287.4,    z:   -22.0 },    // Feathergorge
    AetheryteSpawn { aetheryte_id: 1_280_100, zone_id: 144, x:  1737.0,   y: 176.5,    z: -1250.0 },    // Maiden Glen
    AetheryteSpawn { aetheryte_id: 1_280_101, zone_id: 144, x:  1390.0,   y: 222.6,    z:  -736.0 },    // Hushed Boughs
    AetheryteSpawn { aetheryte_id: 1_280_102, zone_id: 144, x:  1788.0,   y: 164.8,    z:  -829.0 },    // Scarwing Fall
    AetheryteSpawn { aetheryte_id: 1_280_103, zone_id: 145, x:  1383.0,   y: 231.8,    z:   422.0 },    // Weeping Vale
    AetheryteSpawn { aetheryte_id: 1_280_104, zone_id: 145, x:  2160.0,   y: 142.7,    z:   622.0 },    // Clearwater
    AetheryteSpawn { aetheryte_id: 1_280_105, zone_id: 147, x:   -1.0,    y: 144.1,    z:  1373.0 },    // Teriggans Stand
    AetheryteSpawn { aetheryte_id: 1_280_106, zone_id: 147, x:  -64.0,    y: 185.1,    z:  1924.0 },    // Shepherd Peak
    AetheryteSpawn { aetheryte_id: 1_280_107, zone_id: 147, x: -908.0,    y: 191.7,    z:  2162.0 },    // Fellwood
    AetheryteSpawn { aetheryte_id: 1_280_108, zone_id: 148, x: -1734.82,  y: 285.069,  z:  -839.63 },   // Wyrmkings Perch
    AetheryteSpawn { aetheryte_id: 1_280_109, zone_id: 148, x: -2366.07,  y: 336.041,  z: -1054.75 },   // The Lance
    AetheryteSpawn { aetheryte_id: 1_280_110, zone_id: 148, x: -2821.0,   y: 256.1,    z:  -290.0 },    // Twinpools
    // ---- Mor Dhona ----
    AetheryteSpawn { aetheryte_id: 1_280_121, zone_id: 190, x:  487.445,  y:  18.531,  z:   672.244 },  // Camp Brittlebark
    AetheryteSpawn { aetheryte_id: 1_280_122, zone_id: 190, x: -215.76,   y:  18.54,   z:  -668.703 },  // Camp Revenant's Toll
    AetheryteSpawn { aetheryte_id: 1_280_123, zone_id: 190, x: -458.0,    y: -40.9,    z:  -318.0 },    // Fogfens
    AetheryteSpawn { aetheryte_id: 1_280_124, zone_id: 190, x:  580.0,    y:  58.2,    z:   206.0 },    // Singing Shards
    AetheryteSpawn { aetheryte_id: 1_280_125, zone_id: 190, x: -365.724,  y: -18.591,  z:   -25.448 },  // Jagged Crest Cave
];

/// Binary-search the table for an aetheryte id. Returns `None` for
/// unknown ids — `runtime::dispatcher::apply_home_point_revive` treats
/// that as "no usable homepoint" and refuses to warp (HP/state still
/// restored in place).
pub fn lookup(aetheryte_id: u32) -> Option<&'static AetheryteSpawn> {
    AETHERYTE_SPAWNS
        .binary_search_by_key(&aetheryte_id, |a| a.aetheryte_id)
        .ok()
        .map(|i| &AETHERYTE_SPAWNS[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guard against out-of-order edits — `lookup` uses `binary_search`,
    /// so any unsorted entry would silently mis-route a homepoint.
    #[test]
    fn table_is_sorted_ascending() {
        for w in AETHERYTE_SPAWNS.windows(2) {
            assert!(
                w[0].aetheryte_id < w[1].aetheryte_id,
                "AETHERYTE_SPAWNS must be sorted ascending; {} >= {}",
                w[0].aetheryte_id,
                w[1].aetheryte_id,
            );
        }
    }

    #[test]
    fn city_caps_resolve() {
        // The five city CAPs are the most common homepoints — assert
        // their presence as a smoke test.
        let limsa = lookup(1_280_001).expect("Limsa CAP");
        assert_eq!(limsa.zone_id, 230);
        let uldah = lookup(1_280_031).expect("Ul'dah CAP");
        assert_eq!(uldah.zone_id, 175);
        let gridania = lookup(1_280_061).expect("Gridania CAP");
        assert_eq!(gridania.zone_id, 206);
    }

    #[test]
    fn unknown_id_returns_none() {
        assert!(lookup(0).is_none());
        assert!(lookup(999).is_none());
        assert!(lookup(2_000_000).is_none());
    }

    #[test]
    fn child_aetheryte_resolves_to_parent_zone() {
        // Humblehearth (1280067) is a child of the Bentbranch CAP
        // (1280062 → zone 150). The child resolves to the same zone,
        // different coords.
        let child = lookup(1_280_067).expect("Humblehearth");
        let parent = lookup(1_280_062).expect("Bentbranch CAP");
        assert_eq!(child.zone_id, parent.zone_id);
        assert_ne!((child.x, child.z), (parent.x, parent.z));
    }
}
