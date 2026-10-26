# Warrior talents and perfect defense

User approved the class philosophy and requested the full Warrior flow, including perfect block and perfect parry. Implementation began 2026-10-18 and continued 2026-10-19 after the session resumed. Branch: `codex/warrior-talents-perfect-defense`; uncommitted, preserving earlier work and concurrent changes.

## Implemented

- Canonical philosophy: `docs/CLASS-DESIGN.md`. Original design/plan: `docs/plans/2026-10-18-warrior-talents{,-design}.md`.
- Menu > Talents: three compact Arms/Fury/Protection trees, 21 named talents, requirements, ranks, descriptions, available points and defense help. Keyboard arrows/Enter/Esc, mouse and pad D-pad/A/B work through the native menu. Pad Menu closes the whole menu. Opening/re-entering never spends a point with the opening press.
- One point per level 10–60, maximum 51, retroactive for existing characters. Hybrids supported; 26 points minimum for a signature means only one signature fits. Level-up notifications point to the talent menu.
- Per-character `talents-v1.txt`, stable talent IDs, validation of budgets/ranks/gates/class; invalid files are preserved and cannot grant effects. Successful writes precede in-memory changes. Allocation and deliberate reset require a living, idle Warrior outside combat. A reset returns every point and removes signature/temporary benefits.
- Arms: slower rage fade, shorter Charge cooldown, heavy poise/damage, perfect-parry stamina and refreshing non-stacking heavy bleed. Fury: weapon rage, once-per-swing stamina refund and kill healing, light/heavy chain follow-ups, high-rage and low-health damage. Protection: block/parry costs, perfect-block rage, attacker-bound counter damage/poise and chip reduction.
- Mortal Strike: heavy weapon motion, +50% damage/poise, 30 rage, 8 s cooldown. Bloodthirst: light motion, +20% damage and one 5%-maximum-health heal on actual contact, 30 rage, 10 s cooldown. Shield Wall: held shield required, 40% damage reduction for 8 s, 40 rage, 45 s cooldown, timed HUD buff. The signature automatically joins the existing three trained abilities on the four-slot bar; trainers cannot sell or implicitly grant it.
- Signatures validate body, ground, melee motion, full stamina price and shield before payment. Their damage cannot regenerate the rage spent on them. Bleed uses ordinary hit/death/reward messages, without weapon procs or flinches. Passive damage additions are bounded.

## Perfect defense

- **Perfect block:** first eligible frontal melee contact within 160 ms of raising guard; one contact per raise, 450 ms rearm. Cost is 35% of ordinary block stamina, minimum 1. Exact discounted payment is affordable and deals no chip; insufficient stamina falls back to ordinary guard-break rules. Holding/tapping cannot renew a consumed window. Rear attacks, unblockables, grabs, bolts and ground blasts cannot perfect-block.
- **Perfect parry:** preserves the existing authored parry window/volume/front/attack restrictions. Perfect requires the first 120 ms of the player's active window and a hostile attack within 100 ms before its next hit, or during that hit. Earlier valid parries remain ordinary parries; misses retain their stamina/recovery costs. One contact per parry.
- A perfect parry gives +25% to the next actual melee hit on that attacker within 3 s. Protection can improve counter damage and poise after either perfect defense. A weaker block reward does not erase a stronger pending parry counter. Rewards expire and reset on death, class/build changes; stable non-Warrior classes retain the baseline parry opportunity.
- Warriors gain 4 rage from perfect block, 8 from perfect parry, plus applicable talents. Feedback: brief text, gold block/blue parry sparks, metallic impact and short hit-stop. No teleport, automatic hit or invulnerability.
- Also fixed newly created characters publishing only a name: the selected class/race now reaches QuestGame before class/talent systems run, avoiding a fresh Mage temporarily inheriting Warrior behavior.

## Verification so far

- Independent talent specification review and quality review approved; independent combat specification and quality review approved after timing/counter/chain fixes.
- `cargo clippy -j 4 --workspace --all-targets --offline -- -D warnings`: passed.
- `cargo fmt -p wow-souls -p wow-world -p wscl -- --check`: passed.
- `WOW_DATA=... cargo test -j 4 --workspace --offline --quiet`: 1,080 game tests, 259 world tests, 4 integration tests, 1 wscl test and 7 documentation tests passed (1,351 total).
- Production game built successfully. Isolated fixtures/scripts live in `cache/warrior-talents-check/`; no player save was used for mutation.
- Native UI at 1600×900: level-11 Warrior spent one Arms and one Fury point, confirmed a reset, then saved one Fury point. A separate launch loaded exactly one spent/one available point. Final 1280×720 capture confirmed the denser panel background and all controls/help remain on screen.
- All three 26-point builds loaded at level 60 and activated their signatures in the fourth slot: Mortal Strike, Bloodthirst and Shield Wall. Shield Wall showed its 5 s remaining buff and 42 s cooldown at the 13 s capture, and a repeated press refused without another payment.
- Recorded 12 s / 360-frame defense encounters, then inspected dense frame sequences. Perfect parries against `small_1` occurred at enemy 0.733–0.750 s and player 0.033–0.050 s; Warrior rage rose by 8 per success, with text, sparks and the metallic feedback event. An earlier parry remained ordinary in a separate capture.
- Perfect block changed a 29-damage melee hit to 0 with a 4.8-stamina cost and +4 rage. Keeping the same guard held made later contacts ordinary: 30→15 chip before armor, costing 13.6 stamina. This verifies holding guard cannot refresh the perfect window. A separate early raise produced an ordinary block.
- Every live run reported `WOWSOULS_CHECK ok`. Evidence: `C:/Users/jacob/Documents/GitHub/wow darksouls/docs/evidence/warrior-talents/` (UI images, signature frames, ordinary/perfect defense videos/logs, and dense contact sheets). Final panel image: `talents-final-1280.png`.

The player's assessment of combat feel and long-term balance remains a separate playtest.
