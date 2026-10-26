# Road traffic and remains compass — 2026-10-18

Jacob replaced the earlier 14-house experiment with sparse life on the existing
roads, preserving the option to add hidden quests and traders later. He also
asked for the recoverable death location on the compass shown in his screenshot.

## Final scope

- Removed all runtime house placements and vegetation omissions and removed the
  native placement-overlay hook/API created for that experiment. The old survey
  is historical research; rerunning it can no longer overwrite production data.
- `data/world/goldshire-roads.tsv` stores native cobblestone graph vertices and
  connections only. Every original NPC/creature/object and source placement stays.
- Six named friendly adults use stable GUIDs 9100000–9100005 and entries
  9200000–9200005. Four circulate locally; two walk to Stormwind's approach and
  back. They have individual phases and pauses, original appearances and native
  walking/talk clips. No hidden quests, inventory or vendor service is added yet.
- Pure road circuits are sampled from the saved clock (whole days included).
  Visible citizens finish sequential physical corners; they do not teleport to
  catch the schedule. Clock pause also pauses road travel. The spawn ring follows
  live bodies and places absent travellers at their clock-derived route position.
- A native WoW skull marks the saved Bloodstain on an independent compass row,
  with direction/edge arrow and distance at any range. Quest grouping cannot hide
  it. Recovery removes the marker; another soul-bearing death replaces it using
  the existing bloodstain rules. No change to soul loss or saved-file formats.

## Native collision defect exposed by physical road tests

Native terrain uses large baked world coordinates. GJK returned inaccurate
floor normals, and a tangent movement was repeatedly cast into its own floor.
The collision query now translates each triangle and moving shape into a small
local frame, then converts witnesses back correctly. A zero-time capsule contact
that is tangent to or leaving the physical contact normal is skipped after the
existing authored-face and backface-band gates. Every positive-time hit and every
blocking edge/vertex normal is retained. Contact normals, movement speeds, skin,
stepping limits, facing epsilon and backface-band tolerances stay unchanged.
Both sweep and contact-plane queries use the translated frame.

An attempted exact face-normal override was rejected: it introduced three
step/kerb/fence regressions. The narrower final fix preserves Parry's normals.
A standalone adapter includes the actual source motor, swimming, kernel and full
native-road fixture. The staged original kernel passes its 50 existing tests;
the no-gate candidate reproduces Edwin's precise failure. The final t=0 gate
passes all 54, including both native slopes and all six routes both ways.

## Validation

- `cargo fmt -p wow-souls -p wow-world -p wscl -- --check`: PASS.
- `cargo clippy -j 4 --workspace --all-targets --offline -- -D warnings`: PASS.
- `WOW_DATA=... cargo test -j 4 --workspace --offline`: **1317 passed, 0 failed**
  (1052 game, 253 world, 4 world binary, 1 integration, 7 wscl).
- `cargo build -j 4 -p wow-souls --offline`: PASS.
- Scoped engine/survey `rustfmt --check`: PASS. Actual-source independent motor
  adapter: **54 passed**, including 12 native route traversals; negative control
  reproduces the original stall. Source slopes also test translated coordinates.
- The broad gate exposed one stale expectation in the already-added belt-torch
  slot test. Updated count 20→21 and explicitly checked Ammo=19, Torch=20;
  equipment behavior was not changed.
- Live, isolated saves: compass guides to the persisted stain at 350 yd; recovery
  returns 125 souls and removes the bearing. Both runs exit `WOWSOULS_CHECK ok`.
  Original inn scenery and walking/pausing civilians are visible in the town run.
  Final-binary checks `remains`, `travellers` and `roads` also all pass with zero ECS faults and zero stalled-citizen warnings. A nine-frame series (39.75–40.75 s) shows native walking by the inn; Helen progresses through road corners north of Goldshire in the separate traveller check.

Evidence: external `docs/evidence/goldshire-village/` under the user-facing
workspace. Build and gate logs: `cache/goldshire-roads-{fmt,clippy,workspace-tests,build}.log`.
No commit or upload. Original WoW install remains read-only.
Final screenshots:

- "remains/frame-40.png": skull, 350 yd, separate quest/memorial caption.
- "roads/frame-40.png" through "roads/frame-40.75.png": walking civilian by the original inn.
- "travellers/frame-15.png", "frame-20.png", "frame-25.png": Helen approaching along the northern road.
