# Man0u1 "Court in the Sands" (110010) — implementation design

Issue #53. Ul'dah MSQ #2, follows Man0u0 "Flowers for All" (110009, #26).
Sibling ports used as structural templates: man0l1 (#46, Limsa) and
man0g1 (#41, Gridania). This document is the evidence-backed spec the
port was built from; every retail claim below cites its source.

## Evidence base

- **Retail playthroughs (OCR'd via youtube-watcher):** WNRLrwZ3BJY +
  eZgcq-FMpfw (Otto Watt, Lancer, 2-part full run), pGZKU1SHi9M /
  Jcv9I2Bk46w / 6eWqFhITeeM / XbDE5OQ_Y2g / cZvjV5Ilxao (Siddeon
  Tergaru, 5-part full run), 4Brc8QgS4bA (Quicksand intro),
  WlKVCvRgQs0 (17-cutscene compilation, verbatim dialogue).
- **Slerp Lederp dev captures:** sTXkNoVNlRc (his 1.23b Project Meteor
  implementation demo — leaks the internal SEQ numbers via the debug
  `Sequence: NN Class Id: NNNNNNN` onPush line and the six retail
  journal objective texts, verbatim), JQDihRIvkpI (emote segment).
- **Client data:** `bahamut-client-data/lua/scripts/quest/scenario/man/man0u1.lua`
  (113 event functions, text sheet 1359), `csv/man0u1.csv` (full text),
  `csv/quest_marker.csv:198-220` (the 110010xx marker block — doubles
  as a world-coordinate map), `csv/quest_new_reward.csv:13` (rewards).
- **Upstream:** pmeteor quest_system `man0u1.lua` (skeleton + the
  phase-45/50/51/55 header table), `AetheryteParent.lua` 110010 arm.
- **Lore dumps:** Mirke Menagerie Loremonger transcript (61308-62712),
  classic-wiki processEvent table (1681-1810).

## Retail flow → sequence machine

| SEQ | State | Advance | Client events |
|-----|-------|---------|---------------|
| 000 | Quicksand intro instance (PA 175/4). Momodi + 9 ambient NPCs. | Momodi talk → drop to public 175 | `processEvent010` (man0u110 CS), `000_2..10` ambient, `000_1` instance explainer (onNotice) |
| 005 | Run out the Gate of Nald, attune at Camp Black Brush | aetheryte touch (AetheryteParent 110010 arm — already wired) | `processEvent013` (Momodi linkpearl guildleve chain), re-talk `010_2` |
| 010 | Return to Momodi | Momodi talk | `processEvent015` ("The prodigal returns!" + "speak to me again") |
| 012 | Momodi again | Momodi talk → Coliseum Pass 11000126 toast | `processEvent017` (marks Eshtaime's + Coliseum, gives pass); re-talks `017_2/3/4` |
| 015 | Parallel guild legs, counters GSM + GLD | both legs done → NpcLS glow → LS read → 045 | GSM: Elecotte `020` (Niellefresne echo CS, +2,000 gil), re-talk `020_2`. GLD: Yoyobina `030` (man0u130 recruit CS) → GLD lobby PA → Lulutsu `030_2` (pass check) → arena trigger push → `contentsJoinAskInBasaClass` → **coliseum content** (Man0u101) → defeat → `035` (splendid-match CS) → echo PA → Greinfarr `040` (gathering CS) → back to public. Ambient `030_3..11`, `032_2/3/4`, `1000_1/2` |
| 045 | Go to Amajina & Sons | Linette talk | `processEvent050` (man0u150 brawl-intro CS) → miner scene PA |
| 050 | Miner instance #1 — F'lhaminn teaches 6 emotes | 6 emotes done → 057 | teach `051_1..6` (Furious 103, Beckon 108, Laugh 121, Deny 125, Upset 140, Soothe 135 — in order), forgot-replay `051_7(quest,player,npc,51..56)`, go-signal `051_8`; crowd `050_2..14` |
| 057 | Calm the miners: Maddened = Beckon; Manic = Soothe→Furious→Laugh | both calm → ending says (109,107,108,110,111,112,113) → 058 | rebuffs Manic `055_1..3`, Maddened `056_1..3` |
| 058 | Miner instance #2 (reload) — Corguevais recovered | Corguevais talk | `processEvent060` (man0u160 CS: thanks + Linette assignment + F'lhaminn volunteers) → public |
| 060 | Meet F'lhaminn at the Gate of Nald (zone 170) | gate trigger push → confirm → **escort content** (Man0u102) | gate talks (says 144/145/146), duty-start `processEvent070` (man0u170 CS) |
| — | Escort duty: protect F'lhaminn, Gate of Nald → Camp Black Brush. 30-min timer, chinchilla (2204010) ambushes. Runs at SEQ 060 (relog → gate re-arms → retry, no rollback state needed) | arrival → director | banners 51005/50011/25018; barks 365-370/374/375; fail 371/372 |
| 065 | Camp Black Brush (public 170) — the Ascilia echo | Ascilia talk → `080` echo CS → same-zone reload; then camp leader talk → says 176-183 → warp Ul'dah | `processEvent075` (arrival CS, fired by the escort director), `080` (Ascilia/Thancred echo CS), crowd `080_2..12` |
| 070 | Momodi gossip | Momodi talk | `processEvent200_2` (texts 389/390) |
| 075 | F'lhaminn at the Concern stage — escort payment | F'lhaminn talk → +3,000 gil → NpcLS glow → LS read → 080 | `processEvent200` (man0u200 CS, in-place fade — safe) |
| 080 | Frondale's Phrontistery | Nogeloix talk | `processEvent205` (man0u205 CS, in place); re-talk `210_2` |
| 085 | Master Faustigeant | Faustigeant talk → ward door armed | `processEvent210` (man0u210 CS, in place) |
| 090 | Sickroom ward instance (PA 209/5 at the Phrontistery) | ward door push → Warburton echo | ward talks `210_3..8`; `processEvent220` (man0u220 echo CS) |
| 095 | Report to Faustigeant (no pay) | Faustigeant talk → NpcLS glow | `processEvent230` (man0u230 CS, texts 218/219) |
| 100 | Momodi's linkpearl call | LS read → 105 | (server NpcLS texts 307/308) |
| 105 | Turn-in at Momodi | `processEventComplete` + `sqrwa 300,1,1,2` + 200 EXP + 6,000 gil + CompleteQuest | |

SEQ_110 is declared upstream but unused (completion happens at 105,
matching man0g1's shape and Slerp's capture where `Sequence: 105` is
the last pre-completion state).

### NpcLS (linkpearl) message packs — man0u1 sheet ids, sender Momodi (1500014)

1. Post-guilds (SEQ_015 both legs done) → `{301, 302, 303}` (Amajina
   recruiting pitch — verbatim in the retail call) → `SEQ_045`.
2. Cave-in news (SEQ_075 after payment) → `{184, 185, 186}` → `SEQ_080`.
3. Come-see-me (SEQ_100) → `{307, 308}` → `SEQ_105`.

### Rewards (client `quest_new_reward.csv` row 110010 + retail toasts)

- Completion: **200 EXP + 6,000 gil** (`"You obtain 6,000 [gil]"` on
  camera, cZvjV5Ilxao 08:30; the 6,000/200 pair is also in Slerp's
  journal Reward block).
- Mid-quest: **2,000 gil** (Niellefresne, flower — text 44 + retail
  toast) and **3,000 gil** (F'lhaminn, escort — retail toast
  XbDE5OQ_Y2g 10:17).
- Key items: Velodyna Cosmos **11000089** (from man0u0), Coliseum Pass
  **11000126** (granted at `processEvent017`, toast 25117).

## Instance architecture

Retail runs seven "instances" in this quest. Ports:

| Retail instance | Garlemald mechanism |
|---|---|
| Quicksand intro | PA (175, PrivateAreaMasterPast, 4) — already exists |
| GLD lobby + post-match echo | PA (209, PrivateAreaMasterPast, 5) at the Coliseum coords |
| Coliseum arena fight | **content area** `Man0u101` / `SimpleContentMan0u101` / `Quest/QuestDirectorMan0u101` on zone 209 |
| Miner instances #1/#2 | PA (209, PrivateAreaMasterPast, 5) at the Concern coords |
| Escort | **content area** `Man0u102` / `SimpleContentMan0u102` / `Quest/QuestDirectorMan0u102` on zone 170 |
| Camp Black Brush echo | public zone 170 (no PA exists client-side for wil0Fld01 that we can prove; SetENpc gates the cast per-player) |
| Phrontistery ward | PA (209, PrivateAreaMasterPast, 5) at the Phrontistery coords |

PA (209, PrivateAreaMasterPast, 5) is the only Ul'dah-half PA
registered upstream (pmeteor uses it for the pgl200 echo — so the
client provably loads it). All three scene groups share it at disjoint
coordinates; SetENpc arming keeps the casts sequence-scoped.

The client-side `QuestDirectorMan0u101/102` are 8-line empty shells
(same as the shipped Man0l101/Man0g101/102) — all direction is
server-side.

### Coliseum fight (unwinnable by design)

Retail: "You have entered an instance" → *"Duty calls. Do you wish to
proceed with 'Court in the Sands'?"* → arena vs **Tourney Gladiator**
(class 2280157, display 3280156; humanoid — Greinfarr killed the
tourney beasts) → the player IS defeated ("Otto/Siddeon is defeated.")
→ "A splendid match! … Don't let losing get to you." Mirke footnote:
*"There is no way to win."* Uploader: the gladiator is immortal; you
lose by KO or timeout.

Port: the gladiator gets `MinimumHpLock` (can't die); the player gets
`MinimumHpLock` too (no real KO/death UI). The content script treats
player-HP==1 (or a 5-minute timeout) as the loss: emits the worldMaster
"<name> is defeated." line (30121 family), signals the director, which
plays `processEvent035`, bumps the GLD counter, tears down, and warps
to the echo PA. Loss is the completion condition.

### Emote machinery

A player emote reaches the quest `onEmote` hook only when the TARGET's
actor class carries a matching `emoteEventConditions` entry (seed/080
finding). Emote ids are the 100-range EmoteStandardCommand table ids;
the F'lhaminn client-side demo schedulers (0x5000000|((id-100)<<12))
confirm: Furious 103, Beckon 108, Laugh 121, Deny 125, Upset 140,
Soothe 135. `DoEmote(npc, id-100, 21000+(id-101)*10+11)` plays the
player's animation + motion line (formula cross-checked against four
sibling data points: 105→(5,21041), 107→(7,21061), 118→(18,21171),
116→(16,21151)).

- F'lhaminn (teach): all six as emoteDefault1..6 (teach order).
- Manic + Maddened Miner: same six conditions; the script checks the
  step — Maddened wants Beckon (emoteDefault2), Manic wants
  Soothe→Furious→Laugh (emoteDefault6→1→3); wrong taught-emote →
  rebuff bark (`055_x`/`056_x`); untaught emotes never fire (retail
  behavior — the free EmoteStandardCommand handles them).

## Journal map markers (client quest_marker.csv 110010xx block)

11001001 Momodi (-73.4, 78.5) · 11001002 Camp Black Brush (34.8,
-480.5, field map) · 11001005 Eshtaime's/Elecotte (-131.5, 257.0) ·
11001006 Coliseum (-185.3, 179.7) · 11001009 Linette (-92.4, 313.4) ·
11001010/13 F'lhaminn scene (-96.7, 313.2 / -106.2, 318.0) · 11001011
Manic (-108.4, 326.5) · 11001012 Maddened (-107.5, 324.6) · 11001014
Gate of Nald muster (-34.5, -68.9, field map) · 11001015/16 camp
leader (41.1/50.4, -480.0/-481.0) · 11001017 Concern stage (-91.4,
323.2) · 11001018 Phrontistery (-211.7, 279.0) · 11001019 ward door
(-216.4, 302.1). These x/z pairs are literal world coordinates
(verified: 11001009 == Linette's spawn row, 11001018 == Nogeloix's).

## Live-test checkpoints (unprovable statically)

1. `processEvent220` (Warburton echo) is followed by a same-PA reload
   to resolve its after-warp fade; retail appears to leave the player
   free in the same instance. If the reload flickers badly, try the
   in-place `startFadeInCutSceneDefault` neutralizer recipe instead.
2. The gathering echo (`processEvent040`) fires from a Greinfarr talk
   in the echo PA; retail auto-played it on arrival. If auto-play is
   wanted, stage an AfterQuestWarpDirector kick at the 035→echo warp.
3. `processEvent075` vs `080` split: 075 is fired by the escort
   director as the arrival cutscene, 080 by the Ascilia talk at camp.
   If 075 renders as the wrong scene, swap them (both are
   fadeInAfterWarp cutscene drivers).
4. Camp echo runs in PUBLIC zone 170 — the story cast (Ascilia,
   Corguevais, Thancred, camp crowd) is seeded there and visible to
   passers-by. Retail instanced this; acceptable divergence for now.
5. Coliseum "duty party" — retail party list showed the player +
   Abylgo Hamylgo + Papawa during the pre-fight instance. The port
   keeps the arena solo; cosmetic divergence.
6. DoEmote motion-message ids for Laugh/Deny/Upset/Soothe
   (21201/21241/21391/21341) are formula-derived, not capture-proven.
7. **FIXED (2026-07-10 live-run regression):** `SetENpc` is class-keyed
   end-to-end (QuestSetEnpc command → dispatcher claim → streaming
   overlay), so arming `FLHAMINN` (1000038) at SEQ_060 lit BOTH public
   spawns — the gate muster (2435, zone 170) AND the Concern stage
   (2433, zone 209). The live player was pulled to the stage copy in
   the Miner's Guild, got the gate wait-texts 144-146 with no cutscene,
   then never found the 075 payment talk. Fix: seed/099 mints the
   gate-variant class **1099100** (populace recipe, displayNameId
   1900054, appearance cloned from 1000038) and repoints spawn 2435;
   the script arms `FLHAMINN_GATE` at SEQ_060 and keeps `FLHAMINN` for
   the SEQ_075 stage payment. Old saves self-heal: the ENPC set is not
   persisted — login/zone-in re-runs `onStateChange`, which now arms
   only the correct class. Live-verify: `!` over the stage F'lhaminn on
   approach at SEQ_075 (streaming overlay), and NO `!` at the gate copy
   from SEQ_080 on.
8. Accepted divergence: the stage F'lhaminn (2433) is permanently
   present during SEQ_000–070 — retail keeps the stage empty until 075
   ("she rarely sings anymore", csv:292). Despawn/hide isn't viable
   (`area:DespawnActor` is global-to-all-players,
   `GetActorInWorldByUniqueId` is a Nil stub, static actor ids are
   seed-order-fragile); after the seed/099 split she is an unclaimed
   flavour populace NPC there — no icon, no quest dialogue. Harmless.
