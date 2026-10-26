# Elwynn hidden treasures — built and verified

The user approved expanding twelve proposed chests and requested useful equipment discoveries
for every class. Built eighteen Elwynn caches alongside the three Northshire caches: six supply
bundles, four white equipment bundles and eight guaranteed green equipment bundles. Fixed loot,
required levels, class coverage, exact coordinates and discovery clues: `docs/ELWYNN-TREASURES.md`.

## Implementation

- `loot/treasures.rs`: new permanent IDs 9000010–9000027 and fixed `ItemCount` bundles. Existing
  Northshire IDs/rewards are retained. Green cloth Intellect/Stamina, leather Agility and mail
  Strength cover all nine Vanilla classes; the sword/dagger/shield add useful weapon discoveries.
  Warrior/Mage are playable now; this work does not enable unfinished classes.
- `loot/mod.rs`: authored chests use a local downward ray from +1.5 yd over a 3 yd range, waiting
  for their own recorded floor rather than the general spawn snap's +60 yd fallback. This prevents
  streamed mine roofs/surface terrain from capturing an underground cache. Imported baseline
  chest snapping, rewards and restock rules are retained.
- Caches require their reward metadata in the active world manifest. Custom/smaller manifests
  do not award unknown items. Abbey Salves are the existing custom usable item.
- Existing atomic Open/Take/save/claim flow and native chest/interaction animation are reused.
  No quest data, source installs, real character saves or animation clips were edited.
- Removed the pre-existing redundant `dir.into()` in `quests/objects.rs` (String to String),
  which blocked strict Clippy. This is a one-line type cleanup.

## Live verification

Unattended runs used the real motor, native Open/Take animation, terrain/WMO collision and explicit
isolated item saves plus child `LOCALAPPDATA` under `cache/treasure-work/`. Each chest was opened
and claimed, then approached from about 3.5 yd with scripted walking. All eighteen final sites
have successful Open and Take commits with the specified contents. Most geometry checks disabled
NPCs for repeatability; a complete-world Goldshire run also passed with the live manifest, Smith
Argus, guards and other NPCs streaming normally.

Final placement fixes from inspection:

- Ridgepoint's preliminary Z=35 was too high; the local-floor guard correctly withheld it. Z=33.3
  resolves to ground Z=32.9868 and passes approach/Open/Take.
- Westbrook's first location could be looted but a side camera revealed foundation clipping.
  The final site is [-9645,708,37.55], facing southwest, concealed behind native barrels/broken
  crate clear of the outer wall. Final ground Z=37.5465; walk/Open/Take and side-view lid pass.
- The surveyed Heroes' Vigil island site was replaced by the east-bank hut at
  [-9267.23,-1180.15,70.147] so the reward does not require unfinished swimming. The final shore
  site passes walking/Open/Take and grants Veteran Gloves (2980); ground Z=70.1475.
- The Northshire Abbey regression run still places 9000001 at Z=80.2588 and grants three Salves.

The shared all-chest verification save contains exactly eighteen permanent claims, twelve gear
pieces (one each), three Malachite, two Tigerseye and twenty-three Abbey Salves. After a process
restart, repeated interaction presses at a claimed cache left its SHA256 byte-for-byte unchanged.
Final Westbrook and shore relocations were checked again in fresh isolated saves; IDs/content
remain stable, so moving a cache does not reset prior claims.

Evidence/run names in `cache/treasure-work/`:

- `elwynn-<10..27>-walk*`: short walking approaches and native interactions;
- `elwynn-21-walk*`, `elwynn-21-restart*`: corrected Ridgepoint and repeat-claim regression;
- `elwynn-23-final*`, `elwynn-west-final-items.txt`: final clear Westbrook site;
- `elwynn-26-shore*`, `elwynn-shore-items.txt`: final east-bank site;
- `elwynn-10-full*`: complete-world Goldshire; `elwynn-10-abbey*`: Northshire regression.

Representative images/logs are also in the workspace evidence folder `docs/evidence/elwynn-treasures/`.
Earlier `*-side*` / island screenshots are intermediate investigations, not final placements.

## Automated checks

- New regressions check ID independence, smaller-manifest filtering, actual imported class armor/
  primary-stat coverage by level ten, and Warrior/Mage level/equip eligibility.
- The ECS flow test exercises every new fixed bundle through real Open/Take timing, pending loot,
  atomic save/reload, permanent claim and no repeat grant.
- Scoped fmt and strict workspace Clippy passed. Workspace tests with `DSR_DIR` and `WOW_DATA`
  passed: 1,346 (955 game + 241 world + 90 cache + 52 formats + 4 CLI + 4 clips).
- After the final placement-only adjustments, fmt/strict Clippy and the 19 loot-focused tests
  were rerun. Logs: `cache/treasure-work/placements-*.log`; full suite: `final-tests.log`.

Built `target/debug/wow-souls.exe`. No commit requested or made.
