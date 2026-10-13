# Enemy roster: wolf, Defias thief, Garrick, kobold geomancer

Sub-project E2 (2026-10-09). E gave the kobold the Blighttown ghoul's moves. This gives Northshire's other
four creatures a Dark Souls creature each, and turns "the kobold" from code into a row of a table so a creature
is data plus an extractor entry. Nothing is committed; the user commits.

## Why this order, and what the user decided

The user asked (2026-10-09) to fetch similar monsters from Dark Souls and map them to ours, picked the black
dog for the wolf, said "gör allt arbete du behöver", and added a rule (below, R1): a creature that borrows from
a tough Dark Souls enemy gets only one or two of its attacks, so that no ordinary creature is "helt galet svår".
Order: the roster refactor (E2-0), then the wolf first (it is the only new retarget problem: a quadruped, so it
fails early or not at all), then the Defias thief, Garrick, the kobold geomancer.

## The mapping (surveyed in the install, 2026-10-09)

Method: `NpcParam` row names (Shift-JIS) + every map's MSB Enemy part (model name, `NpcParam` id at int 3 of the
type data; checked against the ghoul: `c2060` = `206000`), then `wow-souls-dsr survey --chr cNNNN --npc ID`
(new, read-only: bones, `NpcParam` row, think distances, every clip's length, root travel, TAE event kinds, hit
windows with their `AtkParam_Npc` rows, bullet events). The LLM docs are wrong again: `c2260` is the Lance
Demon (`ヤリデーモン`), not a dog.

| Game creature | Dark Souls creature | chr / NpcParam | DS HP, poise, souls | WoW model |
|---|---|---|---|---|
| Wolf | the black dog (`公使の黒犬`, Undead Burg; an ordinary enemy, not a boss) | `c3340` / 334000 | 84, 0, 80 | `Creature\Wolf\Wolf.m2`, timber skin (display 372 young / 380; by eye) |
| Defias thief | the undead thief (`不死人（盗賊）`) | `c2520` / 252000 | 115, 0, 100 | HumanMale actor with a `CharLook` and a held blade (as the player is built) |
| Garrick Padfoot | the assassin (`アサシン`, Anor Londo) | `c2400` / 240000 | 110, 0, 300 | HumanMale actor, own look, held blade |
| Kobold geomancer | the Oolacile sorcerer (`深淵人魔術師`) | `c4160` / 416000 | 120, 0, 700 | `Creature\Kobold\Kobold.m2` (display 373, 1.15; by eye) |
| Kobold (done) | the Blighttown ghoul | `c2060` / 206000 | 203, 0 | kobold display 365 |

Other quadrupeds in the install, kept as alternatives: the giant rats `c1200`-`c1203` (4-5 attacks), the flame dog
`c3341`, the dragon-kin black dog `c4190`; Sif (`c4520`, `c5210`) is a boss and is not used.

Skeleton facts that drive the retarget work: `c3340` has 58 bones: `LLeg_A_*` = front legs (collarbone, 2 segments,
palm, digits), `LLeg_B_*` = hind legs, `Spine1-3`, `Ribcage`, `Neck1-3`, `Head`, `Jaw`, `Tang1-2`, `Tail1-7`.
`c2520` is the ghoul's naming ("L Thigh", weapon bone `R_weapon`, lower-case w). `c2400` is the player's naming
("L_Thigh", `R_Weapon`) so the existing HumanMale table fits as it is. `c4160` has its own naming ("LThigh1",
"LUpperarm1", "LForearm1", "RWeapon01", skirt and tentacle bones) and needs a table of its own onto the kobold.

## R1. The move budget (the user's rule)

Each creature declares a `Tier`, and the extractor refuses a kit that breaks it (a unit test per kit):

| Tier | Attacks | Rest of the rule |
|---|---|---|
| `Common` | at most 2 | one hit window each, `dmgLevel` 1-2, no knockdown, no bullet barrage |
| `Elite` | at most 5 | multi-hit windows allowed, `dmgLevel` 1-2 |
| `Legacy` | the kobold's 6 as built | not extended; trimming it is the user's call (delete rows of `monster::KOBOLD`) |

Which moves, by the data (damage = `atkPhys x K`, K = 25/138; all level 1 and poise 20 for the dog):

- **Wolf (`Common`)**: `bite` = `a00_3000` (judge 100, hit 0.63-0.87 s of 1.70, 0.65 m of travel, phys 104 = 19 damage; the
  slow readable one) and `lunge` = `a00_3004` (judge 130, hit 0.33-0.70 s of 1.53, 2.07 m, phys 100 = 18). Left out:
  `a00_3001` (4.0 m, phys 140), `a00_3002` (5.1 m, 140), `a00_3005` (2.7 m, 140), `a00_3006` (the fast pounce, hit
  at 0.20 s, no time to read it). If `a00_3004` looks wrong in the sheet, `a00_3006` is the swap, judged by eye.
- **Defias thief (`Common`)**: `a00_3003` (judge 130, hit 0.87-0.97 s of 2.37, phys 150 = 27) and `a00_3004` (140,
  0.73-0.90 of 2.13, phys 153 = 28), both level 1. Left out: `a00_3000`-`3002` (their `dmgLevel` is **8**, which
  `reaction::class_for` would read as a knockdown; the meaning of 8 is not in the params, so they wait until it is
  measured), `a00_3005` (level 2, 3.9 m lunge), `a00_3006` (a thrown bullet, judge-less).
- **Garrick (`Elite`)**: from `c2400`: `a00_3002` (two hits, 0.53-0.77 and 1.17-1.40, phys 140), `a00_3003` (three
  hits, phys 155), `a00_3005` (level 2, 3.5 m lunge, phys 179, hit 0.87-1.00), `a00_3006` (level 2, 4.0 m, 173),
  `a00_3008` (the quick one, 1.37 s, phys 105). Left out: `3000`/`3001` (level 8), `3004` (a backward hop), `3007`,
  `3011` (a bullet).
- **Geomancer (`Common`)**: `bolt` = `a00_3003` (the sorcerer's single bullet, TAE bullet event at 1.83 s of 3.37,
  bullet 200: a long, readable cast) and `poke` = `a00_3000` (staff swing, judge 100, hit 0.90-1.03 of 2.37, phys
  120 = 22, level 1). Left out: `3004` (bullet 210), `3005` (an 8-bullet fan), `3001`/`3002`/`3007` (level 2, 2, 5).

HP, poise and the speeds are game tuning, not the Dark Souls numbers (the ghoul's 203 HP became the kobold's 60):
wolf 40, thief 60, geomancer 40, Garrick 180 with poise 20; all in one table, `species.rs`.

## E2-0. The roster refactor (behaviour-preserving)

Extractor (`dsr-cache`):
- `monster::Kit` gains `tier`, `rig`, `wow_model`, the `DsNames` of its source; `KITS: [&Kit]`. `extract_monster` loops
  over the kits; each writes `<kit.name>/` (WoW bones, local) and `<chr>/` (DS bones, model space); `monster <name> ...`
  lines already carry the name, so the manifest gains blocks, not fields. `EXTRACTOR_VERSION` and `CACHE_VERSION` bump.
- The kobold's block stays line-for-line what it is (a test compares the old and new manifest block).
- `Tier::check` (R1) runs in `Kit::check`.
- DS name styles: `HUMAN_MALE_SPACED` (the ghoul's naming onto HumanMale indices) next to `HUMAN_MALE`.

Game (`wow-souls`):
- `species.rs`: `Species { name, kit, actor (WoW model or human look + weapon), health, souls, speeds, bands, hit volume,
  voice kits, display name, pattern }` and `ROSTER`; `slice::spawn_plan` = a list of `(species, spot)` (the four
  kobolds stay at their four spots; nothing new spawns until a species is built and asked for).
- `npc.rs`, `monster.rs`, `pattern.rs`, `foehit.rs`, `audio.rs` (`VoiceOf`), `hud/enemy.rs` (the name), `dsview.rs`
  (`WOWSOULS_DSVIEW=<species>:<clip>`) read the species instead of `KOBOLD_*` constants.
- Gate: wow-souls 279 / dsr-cache 42 / dsr-formats 51 / wscl 4 still green, the live kobold fight logs the same lines.

## E2-1. The wolf (the quadruped)

The retarget assumes a biped: two legs set the scale, the facing comes from the feet and the shoulders, the hips are
two joints, the soles are two or four bones. Generalising, still bind-aligned delta transfer:
- `Legs.ds` / `.wow` become slices (four legs: front and hind, left and right); the scale is the mean limb length.
- `DsNames.feet` becomes `Facing::FeetToToes(..)` (biped) or `Facing::PelvisToHead(pelvis, head)` (quadruped); left/right
  from the front collarbones (`LLeg_A_Collarbone` / `RLeg_A_Collarbone`).
- `Hips` = the hind-leg joints (`LLeg_B_1`, `RLeg_B_1`) and the hip root; `Checks.cross` = the hind pair and the front pair;
  the contact test uses all four palms.
- WoW side: `benilla-extract m2bones Creature\Wolf\Wolf.m2` gives the indices; the table `WOLF` pairs `LLeg_A_*` with the
  wolf's forelegs and `LLeg_B_*` with its hind legs, spine and neck chain, head, jaw, tail chain.
- The bite's hit volume: a sphere at the jaw/head bone, radius `hit0_Radius` 0.5 DS m scaled by the entity scale.
- Reactions: the black dog has the whole set (`a00_2000`-`2002` 0.70 s, `2004`-`2007` 1.03 s, `2010`-`2013` 1.2 s; launched
  `2014`/`2020`, 2.7 s) and four deaths (`a00_2200`, `2205`, `2207`, `2209`); the kit lists level 1 and 2 flinches and the two
  deaths that pass `check_death`; the rest is left.
- Sound: WoW's own wolf kits through the creature's `CreatureSoundData`, found as the kobold's were.
- If the retarget cannot pass the same gates (direction 15 deg, cross 15 deg, foot gap 0.08), the fallback is WoW's own
  wolf clips with the DS timing (hit window, travel, damage) and the DS pattern; the spec for that is a one-page addendum.

## E2-2. The Defias thief  /  E2-3. Garrick

Humanoid on HumanMale, so the existing `HUMAN_MALE` table and rig (Garrick: as is; thief: the spaced-name twin). The
actor is built as the player is (`CharLook`, a held blade through `HeldItem`), the hit volume is the blade segment between
the held weapon's `$WTT`/`$WTB` markers (the player's `blade_hits`), not the kobold's forearm rule. Garrick needs three
additions: multi-window attacks (`AttackSpec.hit` becomes a list; each window strikes once), poise above 0 (the `Poise`
rule is built and tested), and a boss bar (the DS layout's own, from `menu.drb`; if it has no wide bar, `enemy_HP`
stretched, decided when the HUD module is read). The `WOWSOULS_AT`-style instruments get `_SPECIES`/`_MOVE` so one creature
can be forced.

## E2-4. The kobold geomancer

The kobold rig exists (`KOBOLD_RIG`), the source is a humanoid with its own bone names, so a `GEOMANCER` pair table
(`LThigh1`, `LCalf1`, `LFoot`, `LToe1`, `LUpperarm1`, `LForearm1`, `LPalm`, ...) onto the kobold indices. New: **bolts**.
The extractor reads the TAE bullet event (type 2: params `[dummy poly, 0, bullet id, flag]`) and the `BulletParam` row
(speed, life, radius, attack row) into `bolt` manifest lines; the game spawns a bolt at the cast bone at the event's time,
flies it straight at the player's position at the cast (no homing), tests a swept sphere against the player's hurtbox,
ignores it while `Action::invulnerable()` (so a roll dodges it, like a blow), and draws a WoW spell visual (`spawn_emitter`,
the blood module's way). Damage from the bullet's `AtkParam_Npc` row through K. The geomancer keeps its distance (think
bands near 2 / mid 4 / far 10 m) and pokes only when cornered.

## Verification (as for E)

Per creature: contact sheets of every kit clip beside the DS skeleton (the user judges the look before gameplay is wired);
data-table tests against the extracted clips (clip length, hit window to one frame, travel +-2 %, damage, poise); live runs
with the move forced one at a time (`WOWSOULS_MOVE`), the roll at the telegraph takes no damage, the player's R1 chain kills
it, death pose holds, respawn/reset work; gates fmt, `clippy -j 4 --workspace --all-targets -- -D warnings`, tests with
`DSR_DIR`/`WOW_DATA` exported in the same command; both installs identical before and after.

## Not in this sub-project

Pack behaviour for wolves, a thrown knife (`c2520` `a00_3006`), Garrick's backstab/hop moves, the meaning of `dmgLevel` 8,
Lua AI (the hand-written pattern tables stand in again), quests and spawns (G), armour on the thief (only look + blade).

## Decisions taken without asking (the user was away: "gör allt arbete du behöver")

Wolf first; tiers and the move picks above; HP values; display ids 372 / 373 (the client has no creature templates, so
the exact display of "Young Wolf" / "Kobold Geomancer" is a guess to judge by eye); the thief's `dmgLevel` 8 moves skipped.

## Built: E2-0 and E2-1 (2026-10-09)

E2-0 as specified (the manifest and every clip of the kobold, c2060, human_male and c0000 are byte-identical
before and after; `Tier` is `Common` / `Elite` / `Legacy` with `Tier::check_attack` and a count check in
`Kit::check`; `species.rs`, `monster::Roster`, `slice::spawn_plan`). E2-1, the wolf, with these deviations from
the text above, found while building:

- **One death, not two.** Of the black dog's four deaths only `a00_2205` lies down (it is knocked onto its side);
  `a00_2200` and `a00_2209` sit up on the haunches with the forepaws raised and `a00_2207` is thrown 5.9 m. A
  quadruped on its side is thicker than a biped lying flat, so the extractor's "lies" rule is per kit
  (`Lying`: body under 0.65 of standing and a pelvis settling under 0.08 yd for the wolf; 0.5 and 0.02 for the
  kobold).
- **The scale comes from the hind legs only** (1.274), and the retarget has a **ground lock**. The WoW wolf's legs
  are uniform (0.416 yd per segment) where the dog's are not (0.37 / 0.20 / 0.34 m) and its forelegs are
  relatively longer (1.65 against 1.3), so with the paws compared one to one they sank or hovered by up to 0.3
  yd. Each frame the body is now raised or lowered so the lowest WoW paw stands where the lowest DS digit does
  (`Checks::ground_lock`, `relative_soles`, a per-rig `max_foot_gap`: 0.45 for the wolf, whose worst lift is
  0.30 yd). Bone directions stay exact (drift 0.07 deg).
- The body-axis and shoulder-line cross checks are dropped for the wolf (the WoW front hangs off the root, the
  DS shoulders ride on collarbones); only the hind hip line is checked (worst 8.8 deg).
- The facing of a quadruped is the body's axis, pelvis to head (`Facing::Along`).
- The wolf's bite volume starts at its jaw bone (`Blow::Bone`), not at an attachment of the hand; its swish is
  left out (a bite does not whoosh).
- New tools: `wow-souls-dsr probe` (retarget numbers without the gates) and `WOWSOULS_LOOKDEV=1`; the survey
  prints bind positions.
- Spawn: two wolves for a person playing (WoW -8898,-94 and -8904,-80, open meadow), none in an unattended run.
- Display 372 (the timber wolf at 0.65) is a guess for "young wolf": the client has no creature templates.

Next: the Defias thief, then Garrick, then the geomancer, when the user has judged the wolf (contact sheets
`docs/evidence/wolf-sheet-*.png`, the live death `wolf-sheet-death-live.png`).

## Built: E2-2, the Defias thief (2026-10-09)

As specified (`c2520`, `a00_3003` + `a00_3004`, `Tier::Common`, HumanMale with the player's pairs under the
ghoul's spaced names). Found while building: of the thief's deaths `a00_2202` is still moving at its end, so the
kit takes `a00_2204` and `a00_2200`; the hit volume is the held dagger's blade (`Blow::Blade`, between the
item's `$WTB` and `$WTT` markers) with half DS's hit radius; the spawn spots became a table (`slice::GROUPS`);
armed creatures swish and hit with the sword's flesh impact. The thief is built on the player's body with a look
and a copper dagger (`ItemDisplayInfo` 2704); armour is still not drawn.

## Changed after the user's correction (2026-10-09): WoW's own Defias, human only

The user: use the Defias from WoW (the client has them), only the human race (all that Northshire has),
and make it the right Defias (look it up on wowhead). Researched on classicdb.ch (vanilla; prints the
display id): the Defias of Northshire Valley are the **Defias Thug** (npc 38, `CreatureDisplayInfo` 5035) and
**Garrick Padfoot** (npc 103, display 2073); the valley's wolf is the **Timber Wolf** (npc 69, display 604, 0.55),
not the Young Wolf. So: the "Defias thief" of this spec is the **Defias Thug** (species and kit `thug`; the
Dark Souls source stays the undead thief `c2520`), Garrick wears display 2073, and the actors can wear a
character NPC's gear from its `CreatureDisplayInfoExtra` row (baked body atlas, equipment geosets, helm and
shoulders on their attachments) instead of the made-up look and the naked body. The "armour is not drawn"
limit in "Not in this sub-project" no longer holds for NPCs. The sources say the Kobold Geomancer is in the
Jasperlode Mine (Elwynn), not in Northshire: open question for the user.

## Scope change (2026-10-09, the user): the zone-correct roster, nothing else

"Helt korrekt fiender per zon": Northshire Valley has the Kobold Vermin, Worker and Laborer, the Timber Wolf
and Young Wolf, the Defias Thug and Garrick Padfoot (from the valley's kill quests); **the Kobold Geomancer is
dropped** (so are bolts), the species table gets the three kobolds (one kit, three rows) and the Young Wolf (the
wolf's kit). See the Roadmap in `docs/HANDOVER.md` for the order: all monsters, then quests and XP, then every
item and what is left on the player; parry and block still to check; the spawn rate and the aggression to be
reviewed when every creature is aggressive.


## Built: E2-3, Garrick Padfoot (2026-10-09)

`c2400`, the Anor Londo assassin, five moves (`Tier::Elite`): `double` = `a00_3002` (two hit windows, 0.53-0.77 and
1.17-1.40 s of 2.73), `triple` = `a00_3003` (three: 0.57-0.70, 1.03-1.13, 1.53-1.67 of 2.67), `lunge` = `a00_3005`
(level 2, 3.5 m, hit 0.87-1.00), `dash` = `a00_3006` (level 2, 4.0 m, hit 0.50-0.57) and `quick` = `a00_3008` (1.33 s,
hit 0.33-0.47). The assassin wears the player's bone names, so `HUMAN_MALE_RIG` fits as it is (retarget drift
0.06 deg, cross drift 3.2 deg, foot gap 0.06 yd). Found while building:
- **Multi-window attacks.** The `.wscl` clip already carried every hit window; `monster::AttackSpec.hits` is the
  list (`hit` stays the first, for the telegraph and the lunge's hold), `Brain.struck` is a bit mask
  (`npc::open_window`), `audio` swishes before each window. The extractor refuses a move whose windows carry
  different judge ids.
- **Deaths:** of the assassin's eight, `a00_2204` (backward, 2.37 s) and `a00_2205` (forward, 4.03 s) lie down;
  they still settle 0.024 / 0.017 yd over their last ten frames, so the kit has its own `LYING_ASSASSIN`
  (settle 0.03 instead of the biped's 0.02). `2202` / `2203` settle 0.028, `2207` is thrown, `2209` starts kneeling.
- **Poise.** The Dark Souls row says 0 (every blow flinches him) and every straight-sword move's
  `atkSuperArmor` is 0, so a creature with poise could never be broken. Tuning: `SpeciesDef.poise` (Garrick 20) and
  `hits::light_poise(level)` (a light blow 8, a heavy one 20, more 30): three light blows or one heavy one break
  his swing, and poise is whole again after 5 s unhit.
- **Boss bar:** the layout's own `F20_boss_HP` (667 x 12 layout px at the bottom centre: `BG`, the olive chip
  `HP_bar0`, the red fill `HP_bar1`, the tick frame `frame02`, the end caps `frame01` / `boss_bar_memori4`, name
  and damage text), extracted by `wow-souls-dsr hud` (re-run it), drawn by `hud/boss.rs` while a creature of a
  `Boss` species is engaged and alive (and 2.5 s after it dies, with its last damage); a boss has no bar over
  its head. Without the layout in the HUD cache there is no boss bar.
- **Tuning:** 180 HP (the sources say 102 for a level 5 elite), 80 % of the kit's damage (a triple lands
  three blows: 22 each), chase 5.0, display 2073 with the thug's copper dagger (the client has no creature
  equipment; a guess). Bands in `pattern::GARRICK`, tuned live.
