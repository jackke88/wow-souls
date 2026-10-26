# Hogger rework: a boss that is dangerous and hits with impact — design

Supersedes the kit, pattern and phase parts of `2026-10-13-hogger-design.md`; that document's source facts
(entry 448, spawn, spells, loot, trial character) still hold.

## 1. What the user wants (2026-10-16)

- "han känns inte som en riktig boss ... han behöver vara farlig, det han gör behöver vara impact".
- Source: look in DSR or Elden Ring for a boss to borrow from. The user accepted the recommendation
  (Knight Artorias as the base, an impact package on top) and said: if this does not work, switch to the
  Elden Ring track (§9).
- "Aggression or heavy blows?" was handed to Claude: "ta det som passar bäst in lore-mässigt på Hogger".

## 2. Decision: Hogger fights with **relentless aggression**, punctuated by a few heavy slams (Claude's call)

Lore reasons: gnolls are hyena-men, frenzied pack scavengers, not disciplined heavy hitters. Hogger is the
"huge and terrible" Riverpaw leader whose fame is chasing low-level players across Elwynn; in later WoW
he snarls "Grrr... fresh meat!" and, losing, yelps "Yipe! Help Hogger!" for his pack. A bully: he presses
while he is winning and calls for help when he is not.

What that means in play:
- He does not let go: chains of 2-4 blows, leaps that close the gap (8-15 m in Artorias' own metres, scaled to Hogger; §11.3) when the player backs off to heal, a
  short head butt that breaks a raised guard. Running away and turtling are punished.
- Most blows are medium (about 12-15 % of a level 11 player's health); two or three moves are big slams
  (about 30 %, knockdown, a shock ring on the ground). Those are the impact moments.
- It stays fair: every move has a readable tell, every chain ends in an opening of 1-1.5 s, and the big
  slams have the longest tells. Aggression without tells is a failure (§8).

## 3. Why the current Hogger falls flat (found 2026-10-16)

1. **Wrong source.** The Taurus Demon (`c2250`) is a slow demon built for a narrow bridge: blows land
   1.2-2.0 s into a swing. `tempo 1.4` makes the same slow motion faster, not more threatening. The charge
   travels 3.9 yd.
2. **No ground in the blows.** `monster::keep_judge` drops the shockwave windows (judge 101, 111, ...), so an
   axe hitting the ground does nothing.
3. **Phase 2 is only numbers.** The "roar" is the gnoll's aggro bark played louder (`audio.rs` `on_roar`); no
   clip, no build-up, no blast.
4. Poise is not the problem: the creature's pool is `max(kit poise_max, def.poise)` = the Taurus row's 120.

## 4. The source: Knight Artorias `c4100` (NpcParam 410000: hp 1500, poise 130, variation 41000; think near 3 / mid 6 / far 20 DS m)

- Body: a 130-bone biped (`Master`, `Pelvis`, `Spine`, `Spine1`, `Neck`, `Head`, clavicles, `L/R Thigh/Calf/
  Foot/Toe0`), the same naming as the humanoid sources the gnoll already wears (`GNOLL_SOLDIER_RIG`, from
  `c2550`). A new `GNOLL_KNIGHT_RIG` (or the soldier rig reused) is expected to be small work. 91 of the 130
  bones are cloth (`Cloth_Pelvis_*`, `Tail_b*` on the head = the plume): unmapped.
- Hand: in DS1 Artorias swings with his left hand (the right arm is broken); his skeleton has only
  `R_weapon`. **The builder must probe which hand strikes** (`PROBE_CHR=c4100 PROBE_BONES=...`) and set
  `left_handed` and the blow volume's attachment from what it shows, not from this text.
- Attack clips (survey 2026-10-16; roles are the builder's to confirm from a frame series):

| Clip | Length | Root travel | Hit windows (judge, s) | Likely role |
|---|---|---|---|---|
| a00_3000 | 3.37 s | 3.0 m fwd | 100 @ 0.87-1.00 | single slash |
| a00_3001 | 3.03 s | 2.5 m | 110 @ 0.73-0.90 | quick slash |
| a00_3002 | 3.37 s | 5.5 m | 120 @ 1.17-1.30 | stepping slash |
| a00_3003 | 2.93 s | 10 m | 130+141 @ 1.23-1.33 | leaping strike (gap closer) |
| a00_3004 | 3.37 s | 11 m | 150 @ 0.97-1.37, 160 @ 1.37-1.50 | lunge into follow-up (gap closer) |
| a00_3005 | 5.03 s | 15 m | 170 @ 2.37, 180 @ 2.47 | the flip slam (big slam, longest tell) |
| a00_3007 | 3.87 s | 9.5 m | 190 @ 1.40-1.60 | thrust lunge |
| a00_3008 | 3.87 s | 5.5 m, yaw 1.5 turns | 200 @ 0.50-0.67 | spinning slash |
| a00_3009 | 2.50 s | 5.4 m | 140+141 @ 0.80-0.90 | overhead (big slam candidate) |
| a00_3010 | 2.03 s | 6.5 m **back** | 210 @ 0.87-0.97 | backflip slash (disengage that still hits) |
| a00_3011 | 2.53 s | 0 | 220 @ 0.87-1.17, 230 @ 1.17-1.33 | close two-hit |
| a00_3014 | 3.30 s | 2.5 m | 120 @ 0.83-0.97 | slash variant |
| a00_3023 | 2.37 s | 2.86 m | 240 @ 0.70-0.87 | fast jab (head butt candidate) |
| a00_3012 | 2.47 s | 0 | bullet 500 @ 0.93 | short dark blast around him |
| a00_3013 | 8.03 s | 0 | bullet 510 @ 5.40 | the Abyss power-up: kneels, charges, bursts |

Blows land 0.7-1.4 s into the swing (the Taurus: 1.2-2.0 s). That gap is where "aggressive" comes from:
keep `tempo` at 1.0 and judge first; do not speed the clips up.

## 5. The fight

### 5.1 Kit (tier `Boss`, at most 12 attacks)
About ten Artorias moves: two or three quick slashes and the close two-hit (the chain material), two gap
closers (leap, lunge), the spin, the backflip slash, one fast jab as **Head Butt** (WoW spell 6730: guard
break + stagger), and two big slams (the flip slam and the overhead). Damage from the source AtkParam rows,
tamed to level 11 through the existing species `damage_scale` / `TAMED` path to the bands in §2. Fall-back for
any clip that retargets badly: drop it (the Taurus slam `a00_3013` can stay as a big slam if needed).

### 5.2 Pattern
- Phase 1: chains of 2-3, a gap closer whenever the player is beyond mid range for ~1.5 s (a player who runs
  to drink gets leapt on), the backflip slash after a chain sometimes, Head Butt when the player holds guard.
- Rushing Charge (WoW 6268) stays the opener: the lunge or the leap from range.
- Every chain ends in a recovery the player can punish (1-1.5 s); the big slams leave the longest opening.

### 5.3 Phase 2 at 50 %: Frenzy (replaces the bark-roar)
- The power-up clip (`a00_3013`, 8 s): he drops, howls (a real roar sound, §6) and a red frenzy glow builds;
  the player may punish him during it. At its burst (5.4 s) a blast around him knocks the player back and
  hurts (~20 %) if they are within its radius. A roll through it avoids it.
- Then: chains of 3-4, shorter cooldowns on the gap closers, the short blast (`a00_3012`) as a "get off me"
  move when the player stays close behind him.
- Optional, if `c4500` retargets cleanly: Manus' frenzy flurry (`a00_3022`, 8 s, seven blows) borrowed via
  `BORROWED` as a phase-2 move. If not: longer Artorias chains through `pattern::Style` links.

### 5.4 "Yipe! Help Hogger!" at 25 % (new)
Two Riverpaw gnolls (existing species and kit) run in from his camp side, through the existing summon
mechanism (`named::summons`, the Dockmaster's). They go 30 s after the fight. Once per fight.

## 6. Impact package (applies to every Hogger blow that lands, more to the slams)
1. **Ground shock.** Bring back the shockwave windows for the big slams only, as their own ground hit (radius
   from the source row): it hurts and staggers a player standing in it even when the blade misses. A dust
   ring decal on the ground at the impact point (ground quads, never an `ActorModel` effect) and the existing
   `slam_shake`, stronger.
2. **The player feels it.** Big slams knock the player down (the level 5 knockdown); medium blows send them
   back further than a kobold's; hit-stop on Hogger's blows that land slightly longer than an ordinary foe's.
3. **Sound.** A heavy whoosh on every swing, a heavy impact on blows that land, a ground thud on the slams
   (hit or miss), and a real roar for Frenzy (a WoW creature sound, pitched down if needed; not the bark).
4. **Super armor while he swings.** The player's light blows during his wind-up and hit window do not
   interrupt him; his poise pool (130 from the row) still breaks into a stagger that is a real opening.
5. **Frenzy visuals.** A red glow on him in phase 2 (a WoW spell visual such as Enrage if one fits).

## 7. Non-goals
- No grab / throw on the player (needs a paired player animation; later, if wanted).
- No new music (the Naxxramas playlist stays; the user still to judge it).
- No Riverpaw camp build-out beyond the two adds.
- No change to the player's moveset or to other enemies.

## 8. Anti-goals (a failure even if it runs)
- Unreadable spam: a blow with no tell, or chains with no opening.
- The stretched-body look of 2026-10-13: a torso pulled from the legs, floating poses (the plunge trap).
- A slam that lands on air above the ground, or a leap that floats.
- Damage so high one mistake ends a full-health fight (one big slam ≤ ~35 % of the level 11 trial player).

## 9. Decision gate and the Elden Ring fall-back
After the kit is retargeted, look at the moving fight (dense frame series from the game camera + front view,
per the standing order) before tuning numbers. If Artorias on the gnoll body does not read (proportions,
weight, the flips), stop and report; the fall-back is the Elden Ring track: Leonine Misbegotten or the
Beastmen of Farum Azula (gnoll-like). That needs an ER enemy pipeline (chr binders, regulation AtkParam/
Behavior, ER TAE layout) beyond today's player-only extraction: its own design.

## 10. How it is checked
- Gates: fmt/clippy/tests as in HANDOVER; extractor tests for the new kit (tier, hand, windows inside clips).
- Unattended scripted runs (existing `WOWSOULS_DODGE` / `_ATTACK` / `_ROLL` / trial character at Hogger):
  a passive level 11 player dies in under ~15 s; the phase change and the add call fire once each; every
  ground shock lands at ground height; the frenzy blast misses a rolling player.
- Feel is the user's call, judged in motion: send a short fight clip and frame series.

## 11. Claude's calls after the builder check (2026-10-16; the user may overrule)
1. **Order: gate first.** The Artorias kit is built beside the Taurus one (old kit renamed `hogger_taurus`,
   picked with an env switch until the gate passes). The pattern for it comes next. §9 is judged on that.
   Only then come the impact package (§6), Frenzy (§5.3) and the adds (§5.4). A failed gate leaves the
   current Hogger untouched.
2. **Ground shock and blast are new hit types.** Their radius, knockback and level come from the Artorias
   rows (the shockwave AtkParam rows; Bullet 500/510 and the rows they point at), in DS metres times the
   kit scale, the same as every other reach. They are spheres around the point of impact or around him.
   They are not the reach-and-cone test. A roll's invulnerability frames avoid them, as in DS.
3. **Travel is scaled.** Root motion is Artorias' travel times the kit scale for Hogger's drawn size. The
   "8-15 m" in §2 means source metres: on Hogger the leaps cover what the scale gives (≈ 3-5 yd at the old
   0.307, more with Artorias' human-sized body). Probe the vertical root travel of 3003/3005/3010. A clip
   whose blow lands above the ground is dropped, not compensated (the plunge rule).
4. **Adds.** They are called once, the first time his health crosses 25 % in a fight. Two Riverpaw gnolls
   appear about 15 yd from Hogger, on the far side from the player, and run in. When the fight resets or the
   player dies, they go and the once-flag clears. This is a new health trigger next to `named::summons`, not
   its on-notice timer.
5. **Pierce Armor stays** (his WoW identity). Set a big slam to about 28 % at no stacks, so that three stacks
   (+24 %) stay at about the §8 ceiling of 35 %. The opener is Artorias' lunge or leap with a real blow. The
   Taurus body check (judge 171) goes with the Taurus kit.
6. **Frenzy timing.** The phase change waits for his current move to end, then plays the power-up. During
   it he takes full damage but does not flinch (super armor), and his poise does not break.

## 12. Left to the builder
Exact clip picks and names after the frame series; rig details; damage numbers within the bands; the
sounds and decal assets; pattern weights; whether `GNOLL_SOLDIER_RIG` can be reused.
