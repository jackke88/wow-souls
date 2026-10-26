# Mage: Dark Souls' own magic for the fire casts (2026-10-14)

The user, after playing the first build: "Animationen för eld är inte bra nog ... den känns inte särskilt darksouls.
Har darksouls magik som vi kan ta ifrån när det gäller animation och impact?" Then, to the recommendation:
"Kör på din rekommendation - bara vi får till en bra animation så är jag nöjd".

- **What was wrong**: the first build picked staff-block clips (`a00_6205`) by flag windows; no Dark Souls spell
  plays them, and the release (0.77 s) was a hypothesis. Dark Souls' Fireball is a pyromancy.
- **What Dark Souls has**: `Magic.refType` (the cast motion) picks the clip: `a00_6400 + refType` right hand,
  `a00_6500 + refType` left hand (pyromancy), `a00_6200 + refType` staff. The pyromancy clips carry a type-64
  bullet event (the real release). `Bullet` and `AtkParam_Pc` rows carry the burst and the blow (flinch level,
  poise, push). Details and numbers: memory `wow-souls-ds-magic`.
- **Built**: `dsr-cache`: `tae::BULLET_BEHAVIOR`, `Source::bullet_spawn`, `casts::timing` (bullet event, chain /
  evade flags), `Params::spell_blow`, `casts.txt` version 2 with `impact` lines; the roles Fireball `a00_6505`,
  Fire Blast `a00_6508`, Frost Armor `a00_6506`. Game: `spells::cast::Impact` / `Kit::impact`, the missile's hit
  carries the spell's level / poise / push, the burst takes in neighbours (`projectiles::blast_victims`), a throw
  leaves the left hand's attachment point and burns in that hand only (`Role::throwing_hand`, `hands`).
- **Kept**: WoW's range, speed, damage, models and sounds; Frostbolt's staff clip.
- **To judge in motion**: the 2.0 s Fireball (free from 1.5 s) in a Souls fight; the missile leaving the left
  hand; the burst's heavy flinch and push on a kobold pack; Frost Armor's gesture and crystal.

## Second revision (the second play: "ingen kraft ... animationerna är dåliga")

- **Reference**: the user's video is Elden Ring's all-sorceries showcase (B3CkG1ylo_0). Only DSR is installed, so
  ER's own motions cannot be used; its frames (taken in the browser pane) are the target for weight: the staff
  raised overhead, then a hard swing or thrust with the whole body and a step (Magma Shot, Great Glintstone
  Shard, Glintstone Pebble; the buff Thops's Barrier is the staff held straight up).
- **Match**: Dark Souls' two-handed weapon moves have that shape and the cast clips do not: Fireball `a25_4000`
  (greatsword R1), Frostbolt `a38_4000` (the staff's R1), Fire Blast `a59_4300` (two-handed mace R2), Frost
  Armor `a00_6506` kept. Seen and set aside: great hammer / greataxe R1 / R2 (2.7-3.3 s), greatsword R2.
- **Built**: `casts::release_event` (bullet event, else the first hit window), the three roles, the ensure that
  a cast carries no hit dropped (invulnerability stays refused); game: the missile leaves the staff's tip
  (`HeldBuilt.blade`), `spells::cast_travel` steps the body along the clip's root motion (not a buff), the fire
  burns in both hands again.
- **To judge in motion**: the lunge (1.6-2.0 yd per cast), the 2.5-2.9 s total of Fireball / Fire Blast (free
  from 1.2 / 1.7 s), the missile from the staff tip.
