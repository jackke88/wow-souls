# Next AI: the open work (written 2026-10-18)

Read this, then `docs/HANDOVER.md`. Everything listed under "Done" is built but **not committed** (commit only when
the user asks). The user writes Swedish, short; answer in Swedish. Verify every change live (`WOWSOULS_CHECK` +
`WOWSOULS_SHOT` / `WOWSOULS_RECORD`, see HANDOVER "Build, run, verify"), judge animation from frame series,
never stills. Several source files are **CRLF** (`actor/anim.rs`, the vendored engine): strip `\r` before
multi-line perl edits, or use the Edit tool. Never `cargo fmt --all`. Build `-j 4`.

## What is left (2026-10-19, after the session below)

1. Task 6 cloak/side-view legs: **all changes reverted at the user's request (2026-10-20)**. Keep the original behavior; do not resume without a new request. `docs/log/2026-10-20-cloth-legs.md`.
2. Task 2 rares: Surena and Mother Fang done and live-verified; still: new moves via the extractor for the named mobs that only reuse their base kit (check each in `docs/log/2026-10-17-elwynn-specials.md`).
3. Task 1 walls: verify on the user's case (a mob following into a house).
4. Task 4 scaling: live-checked (see Progress); ordinary mobs still 2-3 swings of a 2H mace — ask the user.
5. Everything below is built and gated (fmt, clippy, 1078 + 259 tests green on 2026-10-19) but NOT user-tested and NOT committed.

## Progress (newest last; update after every step)

- 2026-10-18 **Task 3 Princess DONE**: `npc.rs` rush now `matches!(kit, "boar" | "princess")`; `species::PRINCESS` recovery 0.4 (her health/poise/damage now come from the miniboss rule, task 2; rank stays Normal as in the DB); `pattern::PRINCESS` charge band 2.5-6 (w3), long_charge 6-12. Live (Princess, lvl-11 test char): all 5 moves used, short charge rushes 5.5 yd through, long_charge hits 107, gore 53. Not user-tested.
- 2026-10-18 **Task 7 Geomancer DONE**: `foebolt::CastTell` (sent from `npc::think` at a bullet move's wind-up) puts WoW's precast in both spell hands (`FirePrecastHand`/`IceHand` 0x15+0x16 by `look_of`) and plays precast kit 702 (fire) / 20 (frost) at the caster, cut at the release (`mixer::Level`). `foebolt::windup_rate`: geomancer 0.6 (release 0.53 s -> 0.90 s live). Applies to every caster's tell; only the geomancer is slowed. Not user-tested.
- 2026-10-18 **Task 1 walls PARTLY DONE**: `npc::Unstick` (component, lazily inserted in `move_hostiles`): a travelling creature (not in `Mind::Attack`) that asks > 1 yd/s and gets < 25 % for 0.4 s detours 0.7 s turned 1.2 rad to the more open side (two `cast_camera` probes). Unit test `a_creature_walking_into_a_wall_detours...`; live regression fight: 0-3 detours, swings unchanged. NOT verified on the user's case (a mob following into a house) — stage that next (`WOWSOULS_DEBUG` log line `detours round an obstacle`).
- 2026-10-18 **Task 2 rares PARTLY DONE**: (a) one rule makes every boss-bar non-boss species a miniboss: `species::NAMED_HEALTH` 2.2, `NAMED_DAMAGE` 1.3, `NAMED_POISE` 35 via `SpeciesDef::named()` (used by `max_health`, `blow_scale`, `poise()`); Princess back to health 160 (inherits the rule). (b) Surena (the woman in the farmhouse window, screenshot 9; she and Erlan stand inside the Brackwell farmhouse at wow -9809,-929): her bolt and heavy_bolt are the SAME clip, so `foebolt::windup_rate(species, move)` gives heavy_bolt a 0.45x wind-up (long glowing tell via CastTell) and `bolt_weight` 1.8x damage. (c) Ruklar's overhead gets a feint (`RUKLAR_STYLE`). NOT live-verified: an unattended player at -9806,-932 did not aggro them (probably outside the room) — find an inside spot. Still to do: real new moves per named mob via the extractor (see task 2 notes), Mother Fang has 3 moves.
- 2026-10-18 tests: `cargo test -p wow-souls` 1078/1078 green (fixed: CastTell in `behavior/tests.rs` app, Princess rank back to Normal = DB, `torch-v1.txt` added to `saves::PER_CHARACTER` (now 9 with the other chat's talents)).
- 2026-10-18 **Task 4 scaling DONE (numbers, not live)**: `wow_world::balance::TOUGHNESS` 1.25 -> 1.5, `gear_effect::WEAPON_EXPONENT` 0.35 -> 0.25 (tests updated). Arithmetic, level-10 warrior with Giant Mace (live log `melee x2.88` before): light blow ~49, heavy ~74 after; on-level ordinary (Rockhide, lvl 8-9) ~230 HP = ~4-5 light / 3 heavy; Princess 160 x1.4 x1.5 x2.2 (named) ~ 740 HP = ~15 light / 10 heavy (was 210 = 2.5 heavy). The live Princess fight with `WOWSOULS_HEAVY` did not engage her (player at -9776,-884 hit a porcine) — verify with `WOWSOULS_TRACE=1` once.
- 2026-10-18 **Task 5 souls DONE (first step)**: sales now pay `wow_world::shop::SELL_SOULS_PER_COPPER` = 1 (was the 4 of `SOULS_PER_COPPER`): vendor trash is worth 1/16 of its shop price. Buying, quest copper and kill souls unchanged. Most of the user's 151k came from the 100 %-drop day (fixed earlier). Tests updated (`shop.rs`, `shop/mod.rs` CRLF). Next if still too easy: lower quest copper -> souls, or kill souls for named mobs. Flaky once: `soulbind::tests::soul_binding_interaction_saves_then_emits_success` failed in a full run, passed alone and on two reruns (shared temp dir?).
- 2026-10-20 **Task 6 REVERTED**: user rejected the cloak result and requested the pre-task behavior. Removed all added cloak collision/tether/trace code and run/sprint leg fitting, including their helper modules/tests. Source animation files were never changed. Original cloak behavior restored; earlier Warrior/perception work retained. Do not resume this task without a new request. Historical investigation: `docs/log/2026-10-20-cloth-legs.md`.
- 2026-10-19 **Surena live-verified** (player inside the Brackwell farmhouse: `WOWSOULS_AT=-9803.2,-929.4,40`): close in she swats/slashes (70) with Erlan beside her (axe 50); at range she now alternates bolt (released 0.55 s, 63) and heavy_bolt (1.2 s glowing tell, 113). Needed: `behavior::preferred` no longer forces `bolt` for a caster whose table has `heavy_bolt`; `pattern::SURENA` bolt/heavy_bolt weights 3/3.
- 2026-10-19 **Scaling live-checked**: level-10 warrior, Giant Mace (`gear: melee x2.46` after the exponent change, was x2.88), scripted light swings (`WOWSOULS_ATTACK=1.5,1.2,40`) in the farmhouse: 105 per light swing; Erlan (lvl 8, named) 666 HP = 6-7 swings. Ordinary lvl-9 mobs (~230 HP) still die in 2-3 swings of a 2H mace (1H sword light ~49 = 4-5). If the user still finds ordinary mobs too easy with two-handers, lower the great-hammer moveset multiplier (`weapons.rs` / `anim/source/weapons/<family>/manifest.txt` `multiplier`) rather than creature health.
- 2026-10-19 **Mother Fang 4 moves**: `dsr-cache` `MOTHER_FANG_KIT` own attacks (+ spit `a00_3006`), clip `anim/source/mother_fang{,_reference}/body_attack_07.wsanim`, manifest `attack spit` + `attack-bullet spit`; `pattern::MOTHER_FANG_ELITE` + spit (4-10 yd), styles bite->hop_back, hop_back->spit 70, spit->leap; `foebolt::look_of` venom for her; `CastTell` skips venom spitters (no spell-hand glow). Live (`WOWSOULS_AT=-9045,-618`): spit 55 + her Poison, Web at 7.7 yd, bite. Tests 1078 green.
- 2026-10-19 walls: three runs starting inside the Brackwell farmhouse with Surena+Erlan aggroed, driving out in three directions (`WOWSOULS_DRIVE=1,0|-1,0|0,-1`): 0 detours, they kept striking (8-13 hits), then leashed ("loses interest"). The user's stuck case is NOT reproduced; ask the user where it happened (place + which mob).

## Done this session (context for the tasks)

Ledger: `docs/log/2026-10-18-playtest-batch-3.md`. In short: rarity colours, equipment slot list with stat
deltas, creature anti-stunlock (`Poise::hit_chained`), boar charge x2.2, Hogger damage_scale 1.25, spider slope
tilt (`slope.rs`), Forest Spider spit (kit `tarantula`, `attack spit`), murloc lunge + 3-hit flurry
(`Tier::Frenzy`), nearest-bound-graveyard respawn, swimming (`motor/swim.rs`) + drowning (`breath.rs`), belt
torch (`Slot::Torch`, `torch.rs`, item 9100001), rain puddle splashes (`puddle.rs`, `foot.rs`), rain-indoor fog
1 s (vendored engine `lighting/resolve.rs`).
Last change (2026-10-18): loot = **quest items always, everything else WoW's chance**
(`wow_world::loot::Chances::QuestItems`, `loot::chances()`; `WOWSOULS_LOOT_CHANCES=wow|all`). Done, tested in
`wow-world`, not seen live.

## Tasks, in the user's words and order (2026-10-18 playtest)

### 1. Monsters get stuck on walls ("Buggat att dom fastnar på väggar")
- Screenshot: a creature indoors (a house) stuck against a wall/window. Reproduce: a humanoid caster/rare
  following the player into a Goldshire/Westfall house.
- Look at: creature motor (`npc.rs` `motor::step` ~line 2195, `behavior::safe_direction`), path following
  around WMO walls, `stop_distance`. Likely: chase goes straight at the player; no unstick/strafe when
  `Motor.velocity` stays ~0 while `MoveIntent` asks to move. Add a stuck detector (asked > 1 yd/s, achieved
  < 0.2 yd/s for 0.5 s): sidestep along the wall tangent (the slide normal) or retreat to home.
- Done when: a recorded run where a creature chases the player through a doorway and round a table does not
  stop against geometry.

### 2. Rares and quest mobs must be minibosses ("ska vara svåra. Inte hogger svåra")
- The user: "hon gör ju bara en attack" (a female rare/quest mob in a house, screenshot 9; check Morgaine,
  Surena, Erlan, Clara's Ambusher, any female `caster_female` / `human_bandit_female` / `thug_female` named
  mob); "kolla igenom alla rares och quests mobs (princess, goldtooth) så dom faktiskt har mer än en attack
  pattern".
- The 14 specials and their kits: `docs/log/2026-10-17-elwynn-specials.md` (memory
  `wow-souls-elwynn-specials`). Their tables are in `pattern.rs` (`*_ELITE`, `*_STYLE`), defs in `species.rs`.
- For each named mob: at least 3-4 moves in its table, a combo chain (`then`, `Style`), a gap closer, a
  punish for rolling. Use `Tier::Elite` (max 5 moves, multi-hit allowed) in `crates/dsr-cache/src/monster.rs`.
  New moves need the extractor: `cargo run -j 4 --release --manifest-path crates/dsr-cache/Cargo.toml --
  survey --chr cNNNN` to list a source's moves, add `Attack {..}` to the kit, `extract --out <scratch>`,
  `bake-project --from <scratch> --out <scratch2>`, copy ONLY the new clips into
  `anim/source/<kit>/body_attack_NN.wsanim` and `anim/source/<kit>_reference/...` (rename the `name` line,
  add `# Inspiration - motion from DSR; original clip a00_NNNN.`), and add the `attack ...` (+
  `attack-adaptation`) lines to `anim/source/manifest.txt` by hand. Update
  `species::tests::elwynn_kits_stay_within_the_common_single_hit_attack_budget` and
  `species.rs` ~line 1545 (moves per kit).
- Difficulty: miniboss = between an ordinary mob and Hogger. Raise health / poise / damage_scale in
  `species.rs` (level-1 units, see memory `wow-souls-balance`).

### 3. Princess ("har bara en liten charge ... alldeles för lätt ... dog på två hits med 2 handed mace")
- Table `pattern::PRINCESS` has gore, double_gore, charge, long_charge, rear, but the run-through rush only
  runs for kit `"boar"`: `npc.rs` ~line 1356 `let charging = species.kit() == "boar" && ...`. Let Princess's
  `charge` and `long_charge` use `boar_rush` too (her kit is `princess`), with a longer `RUN` for long_charge.
- She died to two 2H-mace blows: health 120 in `species::PRINCESS` (level 9). Make her ~3x and give her
  poise; check the 2H damage in `wow_world::balance` / `attributes` (AP_EXPONENT 0.5, WEAPON_EXPONENT 0.35).

### 4. Scaling ("Tror vi måste kolla över scalingen fortfarande")
- A level ~8 player with a 2H mace two-shots a level 9 elite. Measure: `WOWSOULS_TRACE=1` logs every blow's
  damage; compare player blow damage vs `SpeciesDef::max_health()` per level. Target: ordinary on-level mob
  4-6 light blows, rare/quest mob 12-20, Hogger as now. Log: `docs/log/2026-10-17-warrior-balance.md`.

### 5. Souls come too easily (screenshot 10: 151,218 souls)
- Sources: kill souls (manifest `souls <kit> N`: 100-150 ordinary, princess 750, hogger 3000), quest copper
  x4 (`wow_world::shop::SOULS_PER_COPPER`), vendor sales (sell price x4). The 100 %-drop day flooded the pack
  with sellable items: that alone may explain it (now fixed). Check the shop prices against income and lower
  `SOULS_PER_COPPER` for sales or quest money; consider a cap. Ask the user what a level-10 should have.

### 6. Running legs look very skewed from the side ("När man springer kollar man från sidan ser benen superskeva ut")
- The player's run is the DS gait (`locomotion.rs`, clips in `anim/source/locomotion.txt`, memory
  `wow-souls-ds-locomotion`). Record from the side: `WOWSOULS_OBSERVE=-2.5,1.2,0 WOWSOULS_OBSERVE_HEIGHT=1.0
  WOWSOULS_DRIVE=0,1 WOWSOULS_GRASS=0 WOWSOULS_RECORD=<mp4>,8,30`, make a strip with ffmpeg (see this ledger).
  Suspects: knee bend/hip yaw from the retarget (`dsr-cache` retarget of the run clip), the sprint straighten
  (`straighten_of`), or the lean (`lively.rs`/`footwork.rs`). Compare with `WOWSOULS_MOTION=1` (remix).

### 7. Kobold Geomancer cast ("lite eld ljud när dom börjar så man får en indikation och lite längre animation")
- Caster bolts: `foebolt.rs`, kit `geomancer` (`dsr-cache` `GEOMANCER_KIT`, borrowed cast). Add a wind-up fire
  sound at the swing start (WoW Fireball precast kit, see `spellvis 133` / `abilities/fx.rs`
  `FirePrecastHand` + its sound) and a fire glow in the hand during the wind-up (`fx::Kind::FirePrecastHand`).
  Longer animation: slow the cast clip's wind-up only (play rate < 1 until the hit window) or pick a longer
  cast clip; keep the release time readable (~1.0-1.2 s).

### 8. Open from before (lower priority)
- White rain fog (screenshot 8, previous batch) not reproduced; needs the user's time/place.
- The overbright/indoor camera-claim cause of grey indoors (`WeatherIndoors` keys on the CAMERA).
- Torch has no flame particles (light only); Forest Spider spits every 2 s if the player stands still.
- Failing test `village::entrance_tests` and clippy errors in `village.rs`, `cartography`, `quests` tests:
  another chat's in-progress work; re-run the gates once it is done.

## Gates before telling the user it is done

```
cd /c/Spel/wow-souls
cargo fmt -p wow-souls -p wow-world -p wscl -- --check
cargo clippy -j 4 --workspace --all-targets -- -D warnings
WOW_DATA="C:/Spel/World of Warcraft 1.12/Data" cargo test -j 4 --workspace --offline
```
