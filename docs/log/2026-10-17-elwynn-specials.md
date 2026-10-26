# SDD ledger — Elwynn specials (own Elite kits, rhythms, boss bar) — 2026-10-17

Spec: `docs/superpowers/specs/2026-10-17-elwynn-specials-design.md` (approved table in chat: "Låter jättebra! ...
Kör på", plus "boss-grafiken på health bar också precis som garrick"). Executor: native (this session).

- Ruling: no separate plan file — the approved table is per-mob data and the mechanisms (kit, `TAMED`,
  `Style`, extraction) are this week's; the spec lists the checks — cost if wrong: none.
- Ruling: 14 mobs, not 13 (Jack and James shared a row in the chat table).

## Boss bar
- `species::BossBar` marker + `BOSS_BAR_SPECIES` (14) + `shows_boss_bar` (every `Rank::Boss` and the list);
  `npc.rs` inserts it; `hud/boss.rs` and `hud/enemy.rs` query `BossBar` instead of `Boss`. `Boss` keeps boss
  mechanics (music, slam shake, phases, Frenzy, pack) and the never-respawn rule: the specials keep their rank.
  Test `the_rares_and_named_wear_the_boss_bar_without_becoming_bosses`.

## Kits (extractor, `crates/dsr-cache/src/monster.rs`)
- The workspace changed under this work (another chat, `2026-10-17-wow-only-dependencies.md`): `dsr-cache` is
  its own workspace (`cargo build --manifest-path crates/dsr-cache/Cargo.toml`), and the project library uses
  neutral names (`a00_3000` → `body_attack_01`, `c2060` → `kobold_reference`, `<kit>_ds` → `<kit>_reference`).
- 14 Elite kits (kit = species name) from the family sources: goldtooth/ruklar/narg (ghoul `c2060`),
  gruff/fedfennel (soldier `c2550`), thuros/morgaine/morgan (thief `c2520`), dead_tooth_jack (player 1H axe
  `a30_*` + kick), james_clark (player sword `a23_*` + kick), erlan (player 2H axe `a30_4*`), surena (staff cast
  + Heavy Soul Arrow `Magic 3020` borrowed + thief slashes), princess (Tusk `c3460`), mother_fang (Crag Spider).
- `TAMED` for every source blow over Elite level 2 (shove, bash: guard_rate 100; thief quick cuts level 8 → 1;
  Princess's Tusk blows; Erlan's R2 level 3 → 2 at phys 143). `Kit::attack` now tames player blows too, and the
  SnapKick adaptation accepts an Elite and any weapon whose PC judge 410 is the zero-damage level 5 kick.
- Ruling: Morgaine and Surena lose the thief lunge (`a00_3005`): on the female body its feet stand 0.193 yd off the
  ground on contact (gate 0.08); Morgaine gets the second quick cut, Surena the second slash.
- Ruling: Morgan steps back with `a00_3500` (0.6 m), not `a00_3301` (its body twists 18.8°, gate 15).
- Ruling: Ruklar's table keeps only the overhead and the shove — his hatchet's light swings pass above the
  player on his 1.3 scale (an existing finding); the kit carries the pair unused.
- Extraction: a fresh extractor build also rewrites some existing folders (kobold, hogger, c0000 ...: the
  vendored engine); only the 14 new kit folders were copied into `cache/dsr`, the manifest diff was additions only
  (406 lines). Backup `cache/dsr/manifest.txt.bak-before-specials`.
- Import into the project library: scratch tool `import-kits.pl` (the renaming rules of
  `tools/rename-project-motions.py`, which is a one-time migration): files renamed with the provenance comment,
  `name` lines, blocks and defattack lines inserted (330 + 62 lines). Backup `cache/anim-manifest.bak-before-specials`.

## Game (`crates/wow-souls`)
- Species point at their kits and tables; `pattern.rs` tables + `Style`s per kit (styles keyed by kit): Thuros
  three-cut chain + lunge opener; Morgaine feints; Morgan cut → step back → lunge; Jack R1 chain + kick + roll
  opener; James R1 + thrust (feint) + kick; Erlan axe flow + heavy pair; Surena bolt/heavy bolt + swat/slash;
  Goldtooth jabs + run-strike opener + pounce; Ruklar overhead/shove; Narg shove → overhead; Gruff pair, triple,
  heavy lunge opener, heavy; Fedfennel bash → heavy/overhead (feint); Princess long-charge opener, double gore,
  rear; Mother Fang bite twice, bite/web → leap.
- Remix rig + feet entries for the 14 kits. Dead tables removed (old GOLDTOOTH, RUKLAR, NARG, ERLAN, MOTHER_FANG).
- Tests: `every_special_has_its_own_rhythm_over_its_own_rows`,
  `the_specials_fight_within_the_elite_budget_of_their_installed_kits`; the Common budget test no longer lists them.

## Live checks (`cache/specials-check/`, scratch `fight-special.sh`: the mob's DB spawn 6 yd north of the level 11
trial character, `WOWSOULS_WORLD_FILE`; a rolling + attacking script, and `PASSIVE=1` with traces)
- Rolling player (75 s): Thuros 18 moves / 14 land (the three-cut chain runs), Narg 12 / 7 (shove → overhead),
  Morgan 15 / 7 (cut → step back → lunge), James 12 / 7 (kick → thrust), Surena 9 bolts (2 heavy), Princess 7 / 4,
  Goldtooth 8 / 4, Jack 8 / 4. Ruklar, Gruff, Mother Fang, Fedfennel, Morgaine, Erlan landed little: the script rolls
  backwards after every wind-up, so the passive runs decide.
- Passive (60 s): Ruklar 5 / 4, Gruff 7 / 7 (triple lands its three), Mother Fang 4 / 2, Morgaine 4 / 3.
- Ruling: Fedfennel's shield bash (`a00_3009`) strikes with the left arm where he has no shield and the blow volume
  is not (0 of 2 landed): replaced by the soldier's quick cut `a00_3006`; the guard break moved to his heavy cut
  (tame of judge 180: same level 2 / phys 212, guard_rate 100). Re-extracted and re-imported fedfennel only;
  inventory rows updated (`body_attack_10` → `body_attack_07`).
- Ruling: Erlan only ever rolled (the rolling cut moves him 0.2 yd, so from inside its 2.0-3.5 band he never
  closed): bands axe 0-2.2, heavy 0-2.4, roll 3.0-4.0 weight 1. Passive: the heavy pair lands (4 / 5).
- Princess observer strip (`princess-sheet.png`): gore, double gore, charges read on the boar body; all 5 moves
  used, 12 landed. Boss bar with the names shown in every strip (`specials-sheet.png`).
- Mid-work another chat renamed `body_body_*` → `body_*` across the library (incl. these kits) and rebuilt the game
  to need `WOW_DATA`; the import tool and the fight script follow both.
- Damage unchanged (species `damage_scale` ≤ 1.0 by the roster test): with the trial gear blows land for 8-23.
  Harder comes from rhythm, chains, feints and guard breaks, as asked ("lite svårare").
- Gates: fmt clean (both workspaces); clippy -D warnings clean (both); tests: wow-souls 1016, wow-world 244 + 4 + 1,
  wscl 7, dsr-cache 95 — all pass.
