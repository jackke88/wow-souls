# SDD ledger — plan: docs/superpowers/plans/2026-10-16-hogger-rework.md

Spec: `docs/superpowers/specs/2026-10-16-hogger-rework-design.md`. Executor: native (this session).

- Ruling: no worktree and no commits — the user's standing order is "commit only when asked", and the
  repo carries other chats' uncommitted work that a worktree would not see — cost if wrong: none
  (the diff stays reviewable in the working tree).
- Pre-flight: T3 produces kit attack names → T4 rows/styles, T6 shock attack names, T7 `frenzy`/`blast`.
  T6 produces `shock::Landed` → T7. Consistent after the plan fix (`pounce`, not `leap`).

## Task 1: survey prints attack rows and bullets
- Survey already printed AtkParam rows with `--npc`. Added: `Params::npc_atk`, `npc_atk_magic`,
  `npc_bullet` (+ `NpcBullet`), survey prints a bullet's row and attack. Output:
  `docs/log/2026-10-16-hogger-artorias-survey.txt`.
- Findings (Artorias, variation 41000):
  - Real shock: `a00_3005` judge 180, 2.47-2.57 s, radius 3.00 DS m, phys 210, level 6, knock 1.6 (the blade
    is judge 170, radius 1.0, phys 250, level 6, poise 300).
  - Judge 141 (`3003`, `3009`) is radius 0.05, phys 100: a point, not a shock.
  - Frenzy blast: `a00_3013` bullet event 5.40 s → behavior 510 → Bullet 41510, radius 7.0 DS m, life 0.5,
    attack phys 0 **magic 220**, poise 300, level 10, knock 2.0.
  - `a00_3012` bullet 500 → 41500 → 41501 radius 0.6, no attack row: a visual only.
- Ruling: §6.1 "big slams" = `flip_slam` (source shock judge 180) and `overhead` (`a00_3009`): the overhead
  has no source shock, so its shock takes judge 180's row numbers at half the radius (1.5 DS m) —
  spec §11.2 says rows; this is the nearest row — cost if wrong: one number.
- Ruling: `blast` (`a00_3012`) has no damage row: dropped from the kit; the "get off me" role of §5.3 is
  `backflip` instead — cost if wrong: one fewer phase-2 move.
- Ruling: the Frenzy blast's damage is the row's magic 220 treated as phys (the game has one damage
  type for foes) — cost if wrong: tuning.
- Task 1: complete (build `cargo build -j 4 -p dsr-cache` ok; survey run read).

## Task 2: probe
- `PROBE_CHR=c4100 PROBE_BONES="L Hand,R Hand"`: `R Hand` carries the greatsword (arcs over 1 m), the left hangs.
  Ruling: knight Hogger is right-handed (`left_handed: false`) — the source's own striking hand — cost if wrong:
  blade on the wrong hand, visible at the gate.
- Rig: the soldier's pairs fit (same biped names, `R_weapon`). Ruling: `GNOLL_KNIGHT_RIG` = soldier rig with
  `max_foot_gap` 0.45 — the ground lock shifts the knight's body a steady 0.17-0.26 yd on every clip (stance
  offset, not per-frame error) — cost if wrong: feet off the ground, visible at the gate.
- Vertical: `probe --kit hogger_knight`: the flips leave the ground (`a00_3005` 1.41 yd over all frames, 0.22 on
  contact; `3008` 0.59; `3009` 0.40); no clip's pelvis ends above the ground. Death `a00_2200`: body 1.44 → 1.10,
  pelvis 0.70 → 0.38 (a kneeling collapse, inside `LYING_BOSS`).

## Task 3: kit `hogger_knight`
- `monster::HOGGER_KNIGHT` (12 attacks incl. `frenzy`), `FOLLOW_JUDGES` + `merge_follow_judges` (cache.rs, Boss
  tier) so `lunge` (150→160) and `close_two` (220→230) keep both blows. Tests: `hogger_knight_wears_artorias`,
  `a_combos_follow_up_window_takes_the_moves_judge`, `the_install_calls_the_knights_chr_artorias` — pass.
- Ruling: tests written right after the const data, not before (data-only const) — cost if wrong: none.
- Ruling: `quick_slash` (`a00_3001`) dropped for the 12 cap (`frenzy` counts) — plan's own fallback.
- Ruling: `frenzy` borrows judge 140's row (it never strikes; its window is past its end).
- Manifest: `monster hogger_knight ... scale 0.5049`, think near 3 / mid 6 / far 20; reach bones 41 32.
  Backups `cache/dsr/manifest.txt.bak-before-hogger-knight`, `cache/anim-manifest.bak-before-hogger-knight`.
  Project manifest diff = only the knight block + `max-error-deg 0.074 → 0.136` + an imported-source line.
- Task 3: complete (`cargo test -p dsr-cache hogger follow_up knights_chr` 5 pass; `cargo test -p wow-souls ownanim::` 5 pass).

## Task 4: species, switch, tables, styles
- `species::HOGGER` = knight (kit `hogger_knight`, `damage_scale` 1.4, chase 4.2, tempo 1.0, recovery 1.0,
  phase two tempo 1.1 / chase 1.15 / cooldown 0.6); `HOGGER_TAURUS` = the old def (not in ROSTER);
  `resolve` / `resolve_with` (`WOWSOULS_HOGGER=taurus`), called first in `npc.rs` placing.
- `pattern::HOGGER_KNIGHT{,_TWO}{,_STYLE}`; `styles()` keyed by kit (`npc.rs:1021` passes `species.kit()`).
- `Temper::HOGGER_KNIGHT`; remix rig + feet for `hogger_knight` (gnoll rig, soles 43 44 52 53).
- Ruling: `resolve_with` matches by kit name, not `ptr::eq` — a `const`'s address is not unique per use (the
  first test run proved it) — cost if wrong: none.
- Ruling: the knight keeps the axe (display 7428, Hogger's WoW weapon) on the greatsword's motion — lore over
  source — cost if wrong: reach reads short; `reach_length` 1.3 kept.
- Task 4: complete (`cargo test -p wow-souls -- species:: pattern:: boss:: footwork:: remix::` → 68 passed).

## Task 5: GATE (in motion)
- Runs: `cache/hogger-gate/` (passive player, game camera `p-*.png` 5-17 s every 0.25 s; side observer `side-*`,
  wide `fl-*` 5.4-6.6 s every 0.1 s). Hogger near the anchor (`WOWSOULS_HOGGERS=1`, `_HOGGER_AT=-8903,-116`).
- Verdict: **reads**. The flip slam is a real somersault (tail and axe sweep the arc, no stretch, lands in
  front of the player, 8.1 yd of travel); the lunge is a low beast lunge with legs and torso together; no
  floating blow. Passive level 11 (250 HP, no gear): 250 → 69 in ~13 s; flip 61 + knockdown 3 s.
- Found: pounce/step_slash/spin missed. Cause = reach, not body: the knight's arcs are spaced for a greatsword,
  the 1H axe's `$WTB..$WTT` blade is 0.43 yd. Ruling: Hogger wields a 2H war axe (display 3367, item 14475) in
  one hand (blade 0.65 yd; the overhead now lands) — visible reach over an invisible stretch (§8) — cost if
  wrong: a different weapon look. Bands: `spin` (0, 2.6) (its blow is at 0.5 s, before its travel),
  `step_slash` (2.4, 4.6) (it overshot a player 1.2 yd off).
- Task 5: complete (gate passed; Tasks 6-9 proceed).

## Task 6: ground shock and blast
- Extractor: `monster::Shock`, `SHOCKS` (flip_slam judge 180 r×1.0; overhead judge 180 r×0.5), `shock_of`,
  `shock_at`, `BLASTS` (frenzy); `cache.rs` writes `attack-shock` / `attack-blast` (bullet → Bullet 41510 →
  AtkParam phys+magic). Manifest: `attack-shock flip_slam judge 180 at 2.467 radius 3.00 phys 210 level 6 knock
  1.60`, `attack-shock overhead ... at 0.900 radius 1.50 ...`, `attack-blast frenzy at 5.400 radius 7.00 phys 220
  level 10 knock 2.00`. Clips unchanged; project manifest = cache + 3 lines.
- Game: `monster::Shock` + `AttackClip.shock/blast` (parse test `a_moves_ground_shock_and_blast_are_read_from_their_lines`);
  new `shock.rs` (pure `centre`, `radius_yd`, `due`, `hits`, `SHOCK_BIT`/`BLAST_BIT` in `Brain.struck`, message
  `Landed`); fired in `npc::strike` once per swing, hit or miss; blade-struck player not struck again by the
  same move's shock; blast centred on the boss.
- Ruling: shock firing lives in `npc::strike` (it has the blade, clock, kit scale) rather than a `shock.rs` system —
  cost if wrong: none.
- Tests: `cargo test -p dsr-cache -- shock hogger` 4 pass; `cargo test -p wow-souls -- shock:: monster:: npc::` 49 pass.

## Task 7: impact + Frenzy
- Ruling: no new super-armor rule — a creature with poise already flinches only when its pool breaks
  (`npc::react`), Hogger's pool is the row's 130 — cost if wrong: he flinches too often (tune in T9).
- Frenzy (`boss.rs`): `FrenzyState` Waiting→Asked→Rising→Done, `frenzy_step` (tests `frenzy_waits_for_the_move_to_end`,
  `frenzy_happens_once_per_fight`), system asks through `Brain.start` + `foot.committed` each frame until the
  move plays; armored (no flinch) while Rising (`npc::react`); roar moved from the threshold to the rise for a
  boss with a Frenzy. Enrage visual (`fx::Kind::Enrage`, `Spells\BeastRageCaster.mdx` on 0x14, 10 s).
- Impact: `boss::on_landed` (shake 1.3× slam / 1.8× blast, Thunder Clap ground burst `ClapBase` scaled to the
  radius, `CLAP_RADIUS` 4.0 a guess); `audio::on_shock` (`Sound\Spells\InfernalImpactBase.wav`, fades by 30 yd);
  roar adds `Sound\Creature\Bear\mBearAggroA.wav` to the gnoll bark.
- Live (`run5`, `run6`, `WOWSOULS_BOSS_HP=0.5`): Frenzy plays after the current move, roar + bear roar, blast
  radius 5.28 yd: clear at 10 yd, at 4 yd "the player is in it" → 54 damage, level 10 knockdown; burst ring shows.
- Open: the burst ring is Thunder Clap's blue-white (no red dust model found yet); swing whoosh unchanged.
- Task 7: complete (`cargo test -p wow-souls -- boss:: shock:: npc::` 30 pass; live runs read).

## Task 8: "Yipe! Help Hogger!"
- `boss.rs`: `PACK_AT` 0.25, `PACK_DISTANCE` 15, `PackCall`, `pack_of` (hogger → `species::OUTRUNNER`),
  `pack_call_due`, `pack_called_after` (clears at full health), `pack_spot` (far side from the player), system
  `pack_call` (only while `Hostile.alert`), yelp `Sound\Creature\Gnoll\mGnollAggro3.wav`; `named::call` and
  `Placing` made `pub(crate)`. Tests `the_pack_is_called_once_at_a_quarter`, `the_pack_comes_from_the_far_side`,
  `a_reset_clears_the_pack_call` pass (boss:: 17).
- Ruling: the adds leave by the Dockmaster's rule (`Summoned`, 30 s after their fight), not at once on a reset —
  existing mechanism, spec §11.4 "they go" — cost if wrong: adds linger 30 s after a wipe.
- Ruling: the call needs `Hostile.alert` (a boss alone at low health calls nobody) — cost if wrong: none.
- Live (`run7`, `WOWSOULS_BOSS_HP=0.2`): one yelp, two Riverpaw Outrunners answer and fight (20 per blow).
- Task 8: complete.

## Task 9: tune, scripted gates
- Misses traced to distance: the knight closed to 1.2 yd and his arcs passed round the player. `stop_distance`
  2.0; bands slash (1.4, 2.8), close_two (1.2, 2.2), head_butt (1.0, 2.0), spin (1.2, 2.8); `backflip` (0, 1.4)
  now opens (weight 1 / 2): the "get off me" move of §5.3.
- Trial character with gear (`--trial-character 11` in a scratch `LOCALAPPDATA`, armour 1067, 290 HP): at x1.4 blows
  were 23-35 (8-12 %). Ruling: `damage_scale` 2.2 (species test ceiling for Hogger 1.5 → 2.2: the knight's rows are
  lighter than the Taurus', x2.2 ≈ the old raw weight) — cost if wrong: one number.
  Result: medium 41-50 (14-17 %), flip slam 55-64 (19-22 %), Frenzy blast 57 (20 %); 5-6 blows kill.
- Ruling: spec §10's "passive player dies < ~15 s" is not met (passive survives ~35 s): every heavy blow knocks down
  with DS knockdown invulnerability, so follow-ups pass through; a scripted roller (0.25 s after each wind-up, R1
  every 1.3 s) dies in ~35 s, took phase two, Frenzy, the pack call (once) and a reset to phase one after death —
  cost if wrong: a softer fight than intended; tune `damage_scale` or bands after the user's playtest.
- Shocks land at ground height (`lands at [.., 82.0x, ..]` = the creature's feet y); blast "clear" at 10 yd,
  "in it" at 4 yd; rolling through the blast not separately scripted (i-frames rule shared with blows, tested pure).
- Evidence: `docs/evidence/hogger-knight-{flip,lunge,frenzy,fight}.png`.
- Gates: scoped fmt clean (touched files formatted with rustfmt only); clippy `--workspace --all-targets -D warnings`
  exit 0; `cargo test --workspace --no-fail-fast` with DSR_DIR/WOW_DATA: dsr-cache 95, dsr-formats 52, wow-souls
  978, wow-world 241, wscl 4 + 7 — all pass (first run failed `a_missile_model_loops...`: Enrage had been marked
  looping; now a flash, cut by `Expires`).
- Task 9: complete.

## Final review (Opus; Fable refused: no usage credits)
- Important 1 fixed: Frenzy queued across a reset. `FrenzyEvent::Withdraw` (Asked/Rising → Waiting on phase one
  clears a pending `brain.start`), `frenzy` ordered `.after(phases).before(think)` (`think` now pub(crate)). Test
  `a_reset_withdraws_a_frenzy_that_was_asked_for` RED (compile) → GREEN.
- Important 2 fixed: a Charge stun cancelled the power-up. `boss::stuns`, checked in `abilities::moves::stun`.
  Test `a_rising_frenzy_shrugs_off_a_stun` RED (compile) → GREEN.
- Important 3 fixed: shock centred at the boss's foot height. `strike` samples the ground under the blade tip
  (`settle::snap_to_ground`), `shock::hits` allows `SHOCK_SLOPE` 0.4 yd per yd. Test
  `a_shock_reaches_a_player_up_a_slope_but_not_one_in_the_air` RED (compile) → GREEN.
- Minor 7 re-graded Important (spec §11.4 says the adds go when the player dies; every wipe after 25 % met them
  again): `PackMember`, `pack_leaves` on `Respawned`; `named::call` returns `Option<Entity>`. Test
  `the_pack_leaves_when_the_player_dies` RED (compile) → GREEN.
- Final: minor (deferred): shock/blast `at` not retimed under `WOWSOULS_MOTION=1` (remix.rs).
- Final: minor (deferred): Taurus switch still calls the pack and plays the bear roar (key `pack_of`/roar on kit).
- Final: minor (deferred): blast poise is the borrowed row's 30, not the bullet's 300 (only with `WOWSOULS_POISE`).
- Live after fixes (`fixrun`): phase two → Frenzy → pack call → blast → overhead shock on sampled ground.
- Gates: own files fmt-clean; `cargo test --workspace --no-fail-fast` all pass (wow-souls 983). Clippy fails only
  on another chat's in-progress `behavior.rs` and its new `SpeciesDef.behavior` field (not this work).

## Deferred minors done (2026-10-17, user: "Gör dessa")
- Remix: `remix::retime_shock` moves a move's shock / blast to the remixed clock (`Clock::target_at`); test
  `a_shock_moves_with_its_remixed_clip`. Live `WOWSOULS_MOTION=1`: flip blade 2.367 → 2.722 s, its shock follows
  (lands after the blade: "no second hit").
- Taurus switch: `pack_of` keyed by kit (`hogger_knight` only), the bear roar only for a boss with a `Frenzy`;
  test `only_the_knight_calls_the_pack`. Live `WOWSOULS_HOGGER=taurus WOWSOULS_BOSS_HP=0.2`: Taurus moves, no yelp,
  no beast roar, no Frenzy.
- Shock / blast poise: `monster::Shock.poise`, manifest lines carry `poise` (flip/overhead shock 40, frenzy blast
  300), `npc::strike` uses it; parse test extended (an old line without poise is refused). Manifest backup
  `cache/dsr/manifest.txt.bak-before-shock-poise`; only the three lines changed.
- Gates: workspace fmt clean, clippy `-D warnings` exit 0, `cargo test --workspace --no-fail-fast` all pass
  (dsr-cache 95, dsr-formats 52, wow-souls 1008, wow-world 241, wscl 4 + 7).
