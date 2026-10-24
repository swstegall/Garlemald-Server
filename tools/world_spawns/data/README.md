# World spawn source data

Inputs for `tools/world_spawns/generate.py`, which writes
`common/sql/seed/101_world_monster_spawns.sql`.

| File | What it is | Source |
| --- | --- | --- |
| `xivl_spawn_observations.csv` | Monster (and other actor) spawns with exact positions, decoded from retail 1.23b packet captures | xivl-captures `derived/spawn_observations.csv` |
| `xivl_mooglebox_markers.csv` | Mooglebox's 2012 regional camp maps: monster, level range, approximate count and map coordinates per camp | xivl-captures `studies/mooglebox-regional-mob-maps/derived/marker-records.csv` |
| `xivl_ge_mob_locations.csv` | Gamer Escape 1.x monster pages: zone, map coordinates, level, behavior | xivl-captures `studies/gamerescape-tables/derived/mob-locations.csv` |
| `xivl_ge_mob_client_crosscheck.csv` | Gamer Escape monster names joined to client actor class ids | xivl-captures `studies/gamerescape-tables/derived/mob-client-crosscheck.csv` |
| `xivl_mob_level_hp.csv` | Monster level and max HP from retail captures | xivl-captures `studies/battle-result-actor-join/derived/mob-level-hp.csv` |
| `height_samples_player.csv` | Ground positions (zone, x, y, z) taken from player movement in local packet logs | this repository's `logs/packet-logs/map-packets.log` |

The `xivl_*` files are copied unchanged from
[XIVLegacy/xivl-captures](https://github.com/XIVLegacy/xivl-captures) at commit
`1fd97b751e474b2024cd7d22277052903a157624`. Their data is licensed
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) by the XIVLegacy Dev Team; the
Mooglebox and Gamer Escape material is credited to those sites as recorded upstream.

## How positions are derived

- Exact capture positions are used as they are.
- Map coordinates convert to world x/z as `100 * map + offset`. The per-region offsets in
  `generate.py` were fitted against NPCs and aetherytes whose world positions the server
  already has (median error about 20 world units):

  | Region | x offset | z offset |
  | --- | --- | --- |
  | La Noscea | -2473 | -2945 |
  | Coerthas | -3263 | -1259 |
  | Black Shroud | -3043 | -3752 |
  | Thanalan | -2640 | -3044 |

- Each camp spreads its recorded number of monsters (at most 16) around the camp point.
- Ground height is blended from the nearest known positions in the same zone, so monsters far
  from any sample can float or sink until real terrain heights are available.
- HP follows the retail level curve in `xivl_mob_level_hp.csv` (80 at level 1).

Run with the database the seed has not been applied to, so the classPath repairs are kept:

```sh
python3 tools/world_spawns/generate.py --db <clean garlemald.db> --client "<FINAL FANTASY XIV 1.23b install>"
```
