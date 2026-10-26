# Elwynn's rares and named quest mobs: own attack patterns + the boss bar — design

## What the user wants (2026-10-17)
"letar upp alla rare-mobs och speciella quest mobs som Goldtooth ... ger dom en unik attack pattern. Så som vi
gjorde med Garrick. Behöver inte vara som Hogger men något som gör dom lite svårare." Approved the table below
("Låter jättebra! ... Kör på") and added: "Vi lägger till boss-grafiken på health bar också precis som garrick."

## Who (13)
Thuros Lightfingers, Morgaine the Sly, Morgan the Collector, Dead-Tooth Jack, James Clark, Erlan Drudgemoor,
Surena Caledon (Defias); Goldtooth, Ruklar the Trapper, Narg the Taskmaster (kobolds); Gruff Swiftbite,
Fedfennel (gnolls); Princess (boar); Mother Fang (spider).

## How
- Each gets its own kit (kit name = species name) at `Tier::Elite` (rule R1: a miniboss, at most 5 attacks,
  flinch level 1-2), taken from the **same DS source its family already wears** (bodies, rigs and retargets
  already verified). Source blows above level 2 are tamed (`monster::TAMED`), a guard breaker gets a high
  `guard_rate`.
- Each gets its own pattern table and `pattern::Style`s (keyed by kit): follow-ups, links, feints, an opener.
- No Hogger mechanics (no frenzy, shocks, adds). Existing named mechanics (`named.rs`: Thrash, Backstab, Gouge,
  Poison, Net, Web) stay and are woven into the styles where noted.
- The family AI (`behavior.rs`, another chat's) keeps choosing when; it picks from these tables. Not edited.

| Mob | Source / body | Signature |
|---|---|---|
| Thuros | thief `c2520`, thug body | three quick cuts in a row (tamed `3000`-`3002`) + lunge `3005` from range |
| Morgaine | thief, female body | feints (held swings catch an early roll); Gouge/Poison as before |
| Morgan | thief | cut and hop back (`3301`), then a lunge in |
| Dead-Tooth Jack | player 1H axe moves (`a30_*`) + kick | brawler: heavy chain + guard-breaking kick |
| James Clark | player straight-sword moves (`a23_*`) + kick | brawler: R1 chain, thrust, kick |
| Erlan | player 2H axe (`a30_4*`) | slow R1 flow, heavy R2 pair, rolling attack as a lunge |
| Surena | caster (staff cast + thief) | a second spell + melee pressure when the player closes |
| Goldtooth | ghoul `c2060`, kobold body | quick in-out jabs, run strike, pounce when the player turns away |
| Ruklar | ghoul | Net → overhead combo |
| Narg | ghoul | holds ground: guard-breaking shove → heavy blow |
| Gruff | soldier `c2550`, gnoll body | the soldier's triple combo `3007` + heavy lunge `3005`; Thrash |
| Fedfennel | soldier | shield bash `3009` (guard break) + heavy overhead |
| Princess | Armored Tusk `c3460` | 11 m charge `3001`, double gore `3002`, rear-up `3009` |
| Mother Fang | Crag Spider `c3400` | Web → leap combo, double bite |

## Boss bar
A new marker `species::BossBar` drives the HUD boss bar (`hud/boss.rs`, `hud/enemy.rs`) instead of
`species::Boss`. Every `Rank::Boss` creature gets it as before (Garrick, Hogger); the 13 get it through a list.
`Boss` keeps meaning boss mechanics (music, slam shake, phases, Frenzy, pack) and respawn rules: the 13 keep
their rank (quest mobs respawn).

## Anti-goals
Unreadable spam (a blow with no tell), Hogger-grade damage, a broken retarget (judge in motion), a quest mob that
no longer respawns.

## Checks
Extractor tests per kit (tier, names, windows); species/pattern tests (rows name kit moves, styles chain and end);
boss-bar test; live: each kit's moves land in a scripted fight, frame strips of the new moves in motion.
