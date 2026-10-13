# Player block, parry and medium-roll verification — 2026-10-09

Built after the user's instruction to focus on parry/block alongside the world/quest work.
The earlier [read-only survey](2026-10-09-guard-parry-survey.md) supplied the source choices.
No commits made; benilla and both game installs remain unchanged.

## Controls and behavior

- Hold **E / pad LB** to raise the Shortsword and block. Release to lower it.
- Press **C / pad LT** for an empty-left-hand parry. The sword itself cannot parry.
- Guard starts protecting at **0.200 s**, from flag 85 on `a10_0100`; `a10_0001` holds
  the stance and `a10_0101` lowers it. Movement is limited to walking, without jumping.
  A moving guard uses the DS upper body over WoW's walking legs; a standing guard uses the full clip.
- Shortsword row **200000**: physical cut **50%**, stability **32**. The prototype computes
  damage × (1 − cut) and guardAtkRate × (1 − stability / 100). A block spends stamina and
  keeps guard, with chip damage, metal impact/sparks and a small shove; it does not cause blood
  or a normal damage flinch. Holding guard pauses stamina regeneration.
- Zero stamina after a block plays `a00_2080..2083`, chosen by the measured direction of the
  source push. Guard break has no invulnerability and clears the buffered action.
- Parry `a42_5100`: **14 stamina**, judge **450**, radius **0.55 m** (0.60 yd), active
  **[0, 7/30 s)**. It has no i-frames. The left hand's swept volume must overlap the enemy's
  hurt capsule. Success interrupts its swing, plays its `a00_2100`, and locks it for
  **1.833 s**, the reaction's flag-41 window.
- Kobold kits (including the variants), thugs and Garrick can be parried. Wolves lack a
  parried clip and are excluded. Pounce is a throw/unblockable and bypasses both defenses.

The front half-plane, stamina formula, regeneration policy and parry eligibility rule are
**prototype choices**, not recovered engine logic. Parry accepts an enemy melee attack from
wind-up through the end of its **final** hit window, including the later hits of a combo.
It does not grant a riposte. Shield equipment, enemy
guard/parry, riposte/backstab and additive guard-hit overlays remain separate follow-ups.

## Cache and extraction

`wow-souls-dsr defense` extends the existing version-6 cache with optional `defense-version 1`,
`defense`, `defparry`, `defclip`, `defgrip`, `defattack`, and per-monster `parried` records.
The ordinary `extract` command also includes defense. Old caches safely omit the feature.
The loader checks finite/ranged values, eight required roles, the required active windows,
absence of defense i-frames, and bone counts against the base roll for both skeletons.
Source size/mtime checks cover the character archives and PARAM/PARAMDEF archives.

All eight player pairs and all three enemy pairs passed the existing retarget gates;
the gates were not relaxed. Player cross drift is at most 3.14°, foot gap at most 0.04 yd.
Enemy cross drift is 6.72° kobold, 10.84° thug, 8.21° Garrick; foot gap at most 0.052 yd.
Extraction log: `cache/defense-work/extract.log` (local, ignored).

## The dodge is medium roll

The pre-existing and current manifest both name **`a00_0710`**. The installed source list:

| Forward clip | Roll | Cached duration | Control cancel | Invulnerability | DS travel |
|---|---|---:|---:|---:|---:|
| `a00_0700` | light | 1.667 s | 0.800 s | 11 frames | 3.94 m |
| **`a00_0710`** | **medium** | **2.033 s** | **1.100 s** | **11 frames** | **3.94 m** |
| `a00_0720` | fat | 2.667 s | 1.600 s | 11 frames | 3.77 m |

These clip/control timings are measured from the user's install. The mod author's
[roll-frame description](https://www.nexusmods.com/darksoulsremastered/mods/615?file_id=2985)
also identifies 24/33/48 control frames for light/medium/fat and 11 invulnerability frames.
The clip duration includes the uncancelled stand-up; movement can resume earlier.

Live, on flat ground with forward input, medium roll moved **4.13 yd in 1.10 s** before
movement cancelled the recovery. The full retargeted root curve is 4.47 yd, with i-frames
**[0, 0.367 s)**. We retained that source clip and timing. An old selector could fall back to
fat roll if medium was missing; extraction now requires the named medium roll and fails
instead. A regression test covers that incomplete list. The prototype uses a fixed medium
roll; equipment-load-based roll selection belongs to the later item/equipment work.

## Verification

Workspace gates with both `DSR_DIR` and `WOW_DATA` set: fmt check, clippy with `-D warnings`,
tests **510 passed** (dsr-cache 53, dsr-formats 51, game 347, wow-world 53 + 2, wscl 4).
The game and extractor build. Installed-data tests exercised the actual guard/parry values,
clip windows, front/back/throw interception and guard-break threshold.

Live unattended checks used `WOWSOULS_WORLD=0`, player `WOWSOULS_AT=-8906,-136`, one foe,
`WOWSOULS_AUDIO=null`, `WOWSOULS_TRACE=1` and explicit
`RUST_LOG=wow_souls=info,benilla_world=warn,bevy=warn,wgpu=warn`.
Every listed check exited zero with `WOWSOULS_CHECK ok`; behavior was separately read from logs:

| Check | Configuration / observation |
|---|---|
| Front block | Kobold at -8904,-136, small_1, guard held: 25 → 12.5 HP, stamina 100 → 86.4; subsequent blocks retain guard. |
| Back hit | Kobold at -8908,-136, guard held: full 25 damage and normal flinch. |
| Throw bypass | Kobold at -8903.3,-136, pounce, guard held: full 22 damage. |
| Stamina break | Garrick's double: each hit costs 40.8; 100 → 59.2 → 18.4 → 0, break2; next hit takes full 20. |
| Kobold parry | small_1, `_PARRY_AFTER=0.5`: success at enemy t=0.595 s, hand t=0.083 s; repeated three times. |
| Thug parry | slash_1, `_PARRY_AFTER=0.6`: success at t=0.699 s, hand t=0.083 s; repeated three times. |
| Garrick parry | quick, `_PARRY_AFTER=0.15`: success at t=0.192 s, hand t=0.021 s; repeated three times. |
| Later combo parry | Garrick double with guard held and `_PARRY_AFTER=1.0`: first hit blocked (20 → 10, stamina 59.2), second wind-up parried at enemy t=0.962 s, stamina 45.2. Repeated at t=0.973 s; the second hit never lands. |
| Late parry | Kobold, `_PARRY_AFTER=1.1`: full 25 damage, no successful parry. |
| Wolf exclusion | Timber Wolf, `_PARRY_AFTER=0.25`: full 19 damage, no successful parry. |
| Release and cancels | Guard walk 2.50 yd/s, lower at 5 s, R1 at 5.5 s, backstep at 7 s; normal transitions and stamina. |
| Guard → roll → guard | With guard and movement held, a roll at 4.5 s returns to raise around 5.6 s at the 1.100 s control threshold; the final trace records 4.16 yd in 1.12 s (one-frame quantization). Trace accounting closes when defense takes over, so subsequent guard walking is not attributed to the roll. |
| Medium roll | No foes, forward drive from 4 s, roll at 4 s: 4.13 yd in 1.10 s. |

Logs and repeatable PowerShell scenarios are in `cache/defense-work/`. The curated live log is
`docs/evidence/defense-checks.txt` in the user's session folder. Pose comparisons are
`docs/evidence/defense-{guard,parry,break}.png` in the user's session folder.
The install metadata comparison found **0 changes** across **175 WoW files** (excluding
`realmlist.wtf`) and **6038 DSR files**.
