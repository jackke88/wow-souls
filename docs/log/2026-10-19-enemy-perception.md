# Syn- och hörselaggro — 2026-10-19

User request: approach closer when enemies face away, using sight and hearing instead of one circular aggro range.

Implemented in `crates/wow-souls/src/npc/perception.rs`, integrated into `npc::think` / `think_step`.

| Source | Detection radius |
|---|---|
| Sight | 20 yd, 140 degree forward cone |
| Standing | Silent |
| Walking (Ctrl, or slow stick) | 2.5 yd |
| Running | 6 yd |
| Sprinting | 10 yd |
| Weapon attack / evade / landing | 8 yd |
| Spellcasting | 12 yd |
| Thunder Clap / Battle Shout | 14 / 16 yd |
| Physical proximity | 1.25 yd, any facing |

Footsteps use actual grounded motor velocity, so wall pushes and airborne travel do not produce footsteps. Actions are gameplay noise regardless of audio volume/device. Both sight and hearing require a clear world ray and at most 5 yd height difference; solid walls are soundproof in this version. Sight is evaluated using the creature's current orientation, including database posts and roaming turns.

Existing combat memory and the 30 yd / 8 yd height leash remain. Stopping or moving behind an already engaged enemy does not clear aggro. Any surviving hit alerts its target even if poise prevents a flinch; this also preserves committed armored attacks.

Initial tuning is shared by families; species-specific hearing and investigation/suspicion are outside this change.

## Verification

- Regression first failed on the old implementation: a silent player 6 yd behind the real NPC system triggered aggro.
- Nine added tests cover front/back/range/cone boundaries, actual movement, airborne/landing, action noise, a solid Avian wall through the real NPC loop, combat memory, return acquisition, dead player and direct-hit acquisition without flinching.
- `cargo fmt -p wow-souls -p wow-world -p wscl -- --check`: pass.
- `cargo clippy -j 4 --workspace --all-targets --offline -- -D warnings`: pass.
- `WOW_DATA=... cargo test -j 4 --workspace --offline`: 1,360 pass (1,089 game, 259 world, 4 integration, 1 wscl, 7 doc tests).
- Binary built with `cargo build -j 4 -p wow-souls --offline`.
- Five isolated live runs all returned `WOWSOULS_CHECK ok`, each with 240 recorded frames / 8 seconds, plus screenshots. Evidence: `C:/Users/jacob/Documents/GitHub/wow darksouls/docs/evidence/enemy-perception/`.

| Live case | Result |
|---|---|
| `front` | Still player 12 yd ahead: `Sight`, facing cosine 1.00, noise 0 |
| `behind` | Same position/distance, enemy turned away: remains Idle throughout |
| `walk` | 2.45 yd/s approach from behind, stop 4.15 yd away: remains Idle |
| `run` | 4.2 yd/s approach from behind: `Hearing` at 5.93 yd, facing -1.00, noise 6 |
| `shout` | Still player behind uses Battle Shout: `Hearing` at 12 yd, facing -1.00, noise 16 |

Inspected the behind screenshot and dense walk/run frame sequences from the game's camera: the walking approach preserves the unaware facing; running makes the enemy turn, approach and attack. Actual user playtest is still pending. The solid-wall case is verified in the real NPC system's headless collision test, rather than a live mine scene.

Prototype-only diagnostic: `WOWSOULS_NPC_FACING=<Bevy yaw radians>` overrides the hand-placed group facing; database spawns keep their authored direction. Reproduction helper and isolated saves: ignored `cache/perception-check/`. Plans: `docs/plans/2026-10-19-enemy-perception{,-design}.md`.

No staging or commits. User playtest/tuning pending.
