# Axe edge clock and dagger block (2026-10-28)

User: "Yxan är på fel håll när man slåss och man kan inte blocka med dagger."

## Axe: head clock

`motion_clip::grip_rotation` turns the held item's blade (`$WTT`/`$WTB` markers) onto the Dark Souls
blade axis by the shortest arc. That fixes the axis and leaves the roll about it to whatever the WoW
model's authoring happens to give. A sword is the same either way up, so it never showed; an axe has
one edge, so its head came out on whichever side the arc left it (the Militia Warhammer had the
same trouble, `2026-10-15-militia-warhammer-grip.md`: "Trail markers describe the shaft and cannot
determine the head's clock around it").

Fix (families `axe`, `axe_2h`, `greataxe` only; every other family keeps the arc mount):

- Item side: `actor::held::build` hands every vertex of the drawn model to `motion_clip::head_side`,
  which returns the direction across the shaft in which the mesh hangs (an axe head's mass is on the
  side of its edge). Stored on `HeldBuilt::edge`. A model that is even all round (a bare blade, a
  double-bitted head) has none and keeps the arc.
- Dark Souls side: `motion_clip::swing_edge` reads, in the weapon bone's own frame, where a point 0.7 yd
  out on the blade travels across the axis through every move's hit windows (an edge strikes first).
  Stored on `WeaponClips::edge`; logged once per family at load.
- Mount: `mount_rotation` = arc onto the DS axis, then a turn about that axis that brings the item's
  head onto the DS edge. Used by `WeaponClips::animated_mount` (moves, stance, guard: the clip's own
  weapon bone) and by `rest_mount` (gaits, rolls, reactions: no weapon bone of their own, so the edge
  the stance holds on its first frame).

## Dagger: block

`guard 0.45 26 0` in the dagger manifests is Dark Souls' own `enableGuard = 0` (a dagger only parries
there). `combat::act` reads that third field as "may not guard", so the guard key did nothing.
`motion_clip::guard_enabled` now opens the guard for `dagger` and `dagger_2h` (all other families keep
what the table says: fist/unarmed still cannot). The block uses the dagger row's own numbers, 45 % cut and
stability 26: it turns a blow, it costs more stamina per block than the sword's 50 % / 32. The guard
clips are the shield guard clips every one-handed family already uses (the manifest names
`shield_guard_raise/hold/lower` for the dagger), so nothing is borrowed. Parry was never gated on the
weapon (only on two-handed).

What the extractor reads for the edge (all axe families, in the weapon bone): `(-1.00, 0.00, -0.04)`,
73 % / 82 % / 96 % of the swing travel agreeing (`axe`, `axe_2h`, `greataxe`); per move, every axe move
of the cache leads with it (`installed_axe_moves_strike_with_the_edge_of_the_fit`: dot 0.76-1.00).
Item side, Hand Axe (display 22101): head side `+Y` in the item frame, 49 % lopsided; Beaten Battle
Axe (8501, 2H): `-Y`, 29 %; the starting sword: none (arc mount, unchanged).

`WOWSOULS_EDGE=0` turns the edge fit off (the old arc-only mount) for an A/B.

## Verification

Scripted recordings (`WOWSOULS_RECORD`, `WOWSOULS_AT=-8906,-136`, `WOWSOULS_OBSERVE=4.2,1.3,0` side
camera, `WOWSOULS_ATTACK` / `_HEAVY` / `_ROLL`), judged as frame series and measured with a temporary
trace (removed again): the cosine, per 60 Hz frame, between the head's edge direction and the way the
blade's tip travels across the shaft, speed-weighted, inside each move's hit window (+-0.05 s):

| move | edge fit off (`WOWSOULS_EDGE=0`) | edge fit on |
|---|---|---|
| Hand Axe R1(0) light | -0.16 | +0.78 |
| Hand Axe R1(1) light | -0.15 | +0.75 |
| Hand Axe R2(0) heavy | -0.09 | +0.95 |
| Hand Axe rolling attack | (not run off) | +0.86 |
| Beaten Battle Axe (2H) R1(0)/R1(1)/R2(0) | (not run off) | +0.92 / +0.92 / +0.99 |

Frame series: with the fit off the head hangs below the handle broadside to the swing; with it on the
blade lies in the swing plane (edge-on from the side) and strikes first. Idle and guard frames show
the head up and forward. Recovery frames (the axe drawn back) read negative, as they should.

Dagger (Dirk, 2139), `WOWSOULS_FOE=garrick@-9056.48,-460.9`, `WOWSOULS_AT=-9066,-460.9`:
- `WOWSOULS_GUARD=6,60`: `defense: blocked 27.0 -> 14.9, stamina -44.4` (45 % cut), repeated blows
  to `guard broken` when the stamina ran out; the guard frames show the raised guard with the dagger.
- `WOWSOULS_DUEL=0`: `defense: parried garrick quick/double` eight times in 40 s with the dagger.
- Garrick standing `Idle` never swings at a still player at 10 yd, so a bare `WOWSOULS_GUARD` with
  no mover shows only `defense hold ... active=true`; the duel bot or a short `WOWSOULS_DRIVE`
  brings him in.

Gates: `cargo test --release -p wow-souls` (with `WOWSOULS_TEST_WEAPONS=anim/source`),
`cargo build --release -p wow-souls`.
