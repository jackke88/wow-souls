# Character selection and UI refinement

The user asked to continue the Elden Ring / WoW UI and specifically improve character selection while Claude works on animations in the same checkout. Changes are limited to UI, menu state and the character preview. No commits requested or made.

## Behavior

- Replaced the title's generic text menu with a bounded six-row character list, individual name/class/race/level, a central 3D preview, a saved profile and a separate Enter world action. Keyboard/pad selection and mouse wheel reach all ten slots. Clicking a character selects it; clicking Enter world starts it. New character, Settings and Quit are separate actions. A full roster hides New character.
- The preview now follows the selected character rather than the active avatar. It reads saved customization, armor/cloak, weapons and shield; missing equipment uses the class's starting outfit. Moving onto a menu command retains the last chosen character and a distinct selection marker. Play starts that profile even when a command has focus; Back from creation returns to that choice. Reading inactive profiles never writes their saves. Existing switch/relaunch and two-step archive behavior are preserved.
- Character creation now permits the supported Mage as well as Warrior. Clicking or typing into the name visibly focuses the field; Enter moves from the name to Create. The primary action says Create character, with an unavailable/invalid-name state. Existing appearance controls and model rotation remain.
- Material panels now have a soft dark backing to keep ivory text readable against bright world scenery. Settings hide the roster to prevent overlapping menu text. A live capture also caught the power meter retaining its startup Rage material tint when Mage activated; the material tint now follows the active class together with its value and gradient. The shared theme and material asset provenance are documented in `2026-10-15-vitals-ui.md` and `crates/wow-souls/assets/ui/prompts.md`.

## Verification

`cargo build -j 4 --offline -p wow-souls --bin wow-souls` passed and updated the normal game executable. Live checks use a copied preview executable, leaving the shared build output unlocked. The roster capture script redirects LOCALAPPDATA to dedicated fixture folders under `cache/ui-vitals`, never the player's real saves. Other UI captures use isolated character/quest/item fixtures. No animation sources were edited for this UI pass.

Navigation tests cover the six-row window for every supported list size, selected inactive Mage appearance, retained choice across creation/back, visible name focus and supported Mage creation.

| Check | Result |
|---|---|
| Scoped format gate | Pass |
| Workspace Clippy, all targets, warnings denied | Pass |
| Full install-enabled workspace tests | Game: 899 pass, 2 animation tests fail; no UI/menu failures |
| Workspace tests excluding exactly those two failures | 1,287 pass: game 899, world 240 + 4 binary, dsr-cache 88, dsr-formats 52, wscl 4 |
| Two saved characters, Warrior/Mage and actual worn/held equipment | Pass, 1600×900 |
| Full ten-character list and window navigation | Pass, 1280×720 and 1920×1080 |
| Ultrawide selector | Pass, 2560×1080 |
| Creation, clickable/typed name, Mage selection; first run | Pass, 1600×900 / 1280×720 |
| Settings overlay and return to retained character/Play | Pass |
| Equipment/inventory grids, system/graphics menus, shop, boss meter | Inspected live |
| Starter quest detail, accept and populated quest log | Pass: Deputy Willem, quest 783; no unrelated foes near the fixture |
| Mage Mana material/value, no DSR cache, casting | Pass, latest captures include the color synchronization fix |
| Warrior kill/XP/level-up | Pass: XP 390 → 442, level 1 → 2, meters update |

The two full-suite failures are in `monster::remix::tests::installed_enemy_blows_keep_the_hand_where_the_clip_puts_it_while_they_can_hit` (`remix.rs:1687`) and `installed_kits_keep_their_feet_arms_and_events` (`remix.rs:1402`), both `Option::unwrap()` on `None` during the parallel animation work. They remain visible in the full-suite result; the separate 1,287-test run excludes only these two names. Logs: `cache/ui-vitals/material-{fmt,clippy,tests}.log`, `ui-confirmed-tests.log`, `ui-final-build.log`.

Evidence is local under `docs/evidence/ui-vitals/`: `roster-{warrior,mage,full-720,full-1080,ultrawide,creation,first-run,settings,settings-back}.png`. Material UI screens use `material-final-{equipment,inventory,system,graphics}.png`; latest shop/boss/dialogue/log and fixed Mana captures use `ui-final-*.png`. Scripted runs report `WOWSOULS_CHECK ok`. Videos and fixtures stay under `cache/ui-vitals/`.

The older `material-final-boss.png` was accidentally overwritten by a later capture inheriting its screenshot environment variable; use `ui-final-boss.png`. The capture script now clears that state. Quest fixtures keep all spawn identities for quest availability but move unrelated NPCs away, so enemy hits cannot close a dialogue during visual inspection. Bank, trainer, reward and death presentation inherit the shared theme but did not receive new full-flow captures in this refinement pass.
