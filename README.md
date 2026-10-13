# wow-souls

A single-player, third-person action prototype in the World of Warcraft 1.12.1 world, built on
[benilla](https://github.com/samwhosung/benilla)'s world engine (Rust + Bevy). It runs with no
server, no login, no network and no WoW UI, and reads the game's data from your own 1.12.1 install
without writing to it. It doesn't touch the retail client.

Design and architecture: [docs/wow-souls-feasibility.md](docs/wow-souls-feasibility.md).
What is copied from benilla, and the pinned commit: [docs/PROVENANCE.md](docs/PROVENANCE.md).

## Layout

```
C:\Spel\
├─ World of Warcraft 1.12\   your install (read-only)
├─ benilla\                  benilla checkout pinned at 2e82d34 (unmodified)
└─ wow-souls\                this workspace
   └─ WoW  ->  ..\World of Warcraft 1.12   (junction, gitignored)
```

## Run

```powershell
cargo run -p wow-souls
```

The first build takes about 10 minutes. The dev build finds the install through the `WoW`
junction. `WOW_DATA=<path to Data>` overrides it.

## Controls

These are the defaults. **Every action's keys, mouse buttons and pad buttons can be changed in the game:
Esc (the pad's Menu button) -> Options -> Controls** (two keys per action, Shift / Ctrl / Alt chords, Delete
clears, Reset restores). The key line at the bottom left always shows the bindings in use. Settings are saved to
`%LOCALAPPDATA%\WoWSouls\settings-v1.txt`.

| Input | Action |
|---|---|
| W A S D / left stick | move, relative to the camera |
| Ctrl | walk |
| Left mouse / F / pad RB | equipped weapon's Souls R1 chain, or a committed ranged shot |
| R / right mouse / pad RT | heavy attack, R2 (press again for its follow-up); the mouse buttons need the cursor captured |
| Hold E / pad LB | weapon block with its DSR cut/stability; protects the front and costs stamina on impact |
| H | toggle 1H/2H grip for dagger, sword, axe or mace; large two-handed weapons stay two-handed |
| Y / pad D-pad left | draw / put away the equipped ranged weapon |
| C / pad LT | parry with the empty left hand; costs 14 stamina, active for 7 frames at 30 fps; a miss has recovery and no i-frames |
| X / pad Y | open a nearby chest, then press again to take its contents; collect corpse loot |
| Tab (pad: Esc / Start -> Pack, or bind a button) | open / close inventory; movement and combat input are held while it is open; damage closes it |
| Up / Down / W / S / pad D-pad | select an inventory item; mouse clicks also select rows |
| Enter / pad A / Use button | use the selected Abbey Salve; on a wearable item, equip it (WoW's quick equip: the first free legal slot) |
| G / pad Select | the equipment page: WoW's character pane, all 19 equipment slots and the ammo slot (see Equipment below); G again closes it, Tab goes to the pack |
| Up / Down, Left / Right (or [ ]), pad D-pad, pad LB / RB | on a quest offer, hand-in or in the quest log: look at the reward items (an icon each); the card on the left shows the item's tooltip and how it compares with what you wear in that slot. With several choices, Enter / A picks and a second press hands in |
| V / pad X | quick-use an Abbey Salve while alive, idle and hurt (restores up to 30 health) |
| Escape / pad B / Close button | close inventory (on the equipment page: first leaves a slot's list) |
| Space / pad B | roll along the stick; with the stick still, backstep; a light press just after it is the rolling attack |
| Shift+Space / pad A | jump |
| Mouse / arrow keys / right stick | orbit the camera |
| Q / middle mouse / right-stick click | lock on to the enemy in front (20 yd); press again to let go; with nothing in reach, recentre the camera |
| Esc / pad Start | the menu: Resume, Pack, Equipment, Quest log, Options (Sound, Graphics, Controls, Gamepad, Game), Quit game; the world stands still while it is open (an option), Esc / B steps back |
| F1 | toggle the free-fly debug camera (hold right mouse to look, WASD, Space/C, Shift) |
| F2 | draw the collision capsule and the blade segment |
| F3 | hide the dev readout and the help line |
| F6 | every creature home again, whole and calm (to fight them again without dying first) |

An Xbox pad needs no setup: switch it on before starting the game. The help line at the bottom
left says `pad: <name>` once the game sees it, `(last RB)` after each button, and `pad: none` if it
does not. (benilla turns Bevy's gamepad plugin off; `boot.rs` turns it back on.)

## Switches

A run with `WOWSOULS_CHECK`, `_SHOT`, `_DRIVE` or `_SPIN` set is unattended: its window opens
unfocused below other windows, never captures the cursor, and ignores live input.

| Variable | Effect |
|---|---|
| `WOWSOULS_AT=x,y[,z]` | open at another WoW world position |
| `WOWSOULS_WIN=WxH` | window size (default 1600x900) |
| `WOWSOULS_SHOT=<png>` | write one frame at `WOWSOULS_SHOT_AT` seconds (default 25), then exit; several times (`6,6.1,6.2`) write `<name>-<s>.png` for each (a gait cycle in one run) |
| `WOWSOULS_CHECK[=secs]` | run for a bounded time, exit non-zero on any ECS fault |
| `WOWSOULS_DRIVE=x,y[,s]` | hold a constant stick push (x right, y forward), jumping every `s` seconds |
| `WOWSOULS_SPIN=<rad/s>` | orbit the camera at a constant rate |
| `WOWSOULS_TRACE=1` | log the body (4×/s) and the camera boom, sweep hit and body fade (2×/s) |
| `WOWSOULS_OBSERVE[=x,y,z]` | render from an observer offset (default overhead) with the camera rig drawn |
| `WOWSOULS_BG=0` / `=1` | force an attended / unattended run |
| `WOWSOULS_ATTACK=<s>[,<every>]` | press attack (R1) at `s` seconds, then on a period |
| `WOWSOULS_HEAVY=<s>[,<every>]` | press heavy (R2) at `s` seconds, then on a period (only with the Dark Souls moveset) |
| `WOWSOULS_ROLL=<s>[,<every>]` | press evade at `s` seconds, then on a period (`WOWSOULS_DRIVE` aims it) |
| `WOWSOULS_GUARD=<from>[,<until>]` | hold guard during this interval (real seconds); omitted end holds it for the run |
| `WOWSOULS_PARRY=<s>[,<every>[,<until>]]` | scripted parry presses |
| `WOWSOULS_PARRY_AFTER=<delay>` | press parry this many real seconds after each enemy telegraph, for timing verification |
| `WOWSOULS_LOOT=1` | enable chests in an unattended check (enabled normally for a person playing) |
| `WOWSOULS_INTERACT=<s>[,<every>[,<until>]]` | scripted chest / corpse interaction presses |
| `WOWSOULS_INVENTORY_AT=<s>` / `WOWSOULS_USE_AT=<s>` | scripted inventory open / salve use |
| `WOWSOULS_GEAR_AT=<s>` | open the equipment page at `s` seconds; `WOWSOULS_GEAR_CURSOR=<slot key>` puts the cursor on a slot, `WOWSOULS_GEAR_CHOOSE=1` opens its list, `WOWSOULS_GEAR_SCRIPT=<s>:<up\|down\|left\|right\|confirm\|back\|takeoff>,...` presses keys for an unattended run |
| `WOWSOULS_EQUIP=<item>[,..]` / `WOWSOULS_EQUIP_AT=<s>` | add items to the pack and put them on once (after `s` real seconds); `WOWSOULS_UNEQUIP=<slot key>[,..]` takes slots off first; `WOWSOULS_GIVE=<item>[,..]` only fills the pack; `WOWSOULS_ITEMS_EQUIP_AT=<s>` presses equip on the pack's selected item |
| `WOWSOULS_WEAR=<slot>:<display>[,..]` | the body wears these `ItemDisplayInfo` ids whatever the save says (slot keys such as `head`, `shoulders`, `chest`; `cloak`, `shield`, `offhand`, `mainhand`), for looking at gear the valley does not hand out |
| `WOWSOULS_KIT=0` | do not dress a new character in WoW's starting kit |
| `WOWSOULS_GEAR_EFFECT=0` | worn gear does nothing in a fight (default on: armor lowers damage taken by WoW's formula, a weapon's dps scales the Dark Souls moveset's damage, a shield's block value shrinks a blocked blow; see `gear_effect.rs`) |
| `WOWSOULS_RANK=<species>:<rank>[,...]` | make a species `normal`, `rare`, `miniboss` or `boss` for this run (a boss or miniboss never respawns and stays dead in `world-state-v1.txt`; a boss or rare drops the best item of its table the first time) |
| `WOWSOULS_WORLD_STATE=<file>` | the never-respawn / first-kill / held-loot save (`world-state-v1.txt` beside the other saves by default; an unattended run without this switch keeps it in memory only) |
| `WOWSOULS_ITEMS_SAVE=<path>` | item/chest save override; unattended checks otherwise get an isolated file |
| `WOWSOULS_LOOT_SEED=<n>` | fixed corpse-loot seed for verification; normal play rolls once per death |
| `WOWSOULS_DODGE=<d>[,x,y]` | press evade `d` seconds after each enemy wind-up, the stick `(x right, y forward)` in the body's frame (none: backstep) |
| `WOWSOULS_LOCK=<s>` | press the lock button once at `s` seconds |
| `WOWSOULS_KOBOLD_AT=x,y[,z]` | move the first kobold (WoW coords: +x north, **+y west**); the others stand at fixed offsets from it |
| `WOWSOULS_KOBOLDS=<n>` | how many kobolds (0 to 4; default 4 for a person playing, 1 in an unattended run or when `WOWSOULS_KOBOLD_AT` is set, so the scripted checks keep a lone kobold) |
| `WOWSOULS_RESET=<s>` | press the F6 reset once at `s` seconds |
| `WOWSOULS_SHOT_DEATH=<s>[,<s>...]` | with `WOWSOULS_SHOT`: write the frame `s` seconds after the player's first death instead of at a fixed time (several: `<name>-<s>.png` each), for the YOU DIED banner |
| `WOWSOULS_HUD=0` | the plain HUD (coloured rectangles) instead of Dark Souls' own |
| `WOWSOULS_HUD_GAIN=<x>` | multiply the colour of the HUD's art (Dark Souls' bars are dark; default 1.0) |
| `WOWSOULS_SHOW_BODY=1` | start with the F2 overlay on |
| `WOWSOULS_LOOK=s,f,h,c,b` | the human male's skin, face, hair style, hair colour, facial hair (default `2,1,3,1,1`) |
| `WOWSOULS_DSVIEW=<clip>` | side-by-side: a cached Dark Souls clip (`1` is the roll, `a23_3000` or `r1_1` a sword move, `walk` / `run` / `sprint` / `lock_run_l`... a gait clip) on its own skeleton (gold gizmo bones and a pale blade, 1.5 yd to the avatar's right) beside the human playing the retargeted clip, looping in place; a magenta line lays Dark Souls' blade axis over the WoW sword; logs `dsview t=` 10×/s and, per loop, `blade error` (the WoW blade against Dark Souls') |
| `WOWSOULS_GRIP=rx,ry,rz` | rotate the held sword (degrees about the hand's axes) while Dark Souls clips pose the body, instead of the automatic fit onto Dark Souls' blade axis |
| `WOWSOULS_DSVIEW=kobold:<clip>[@<s>]` | the kobold plays a kit clip (`small_1`, `big`, `run_strike`, `body_check`, `pounce`, `small_2` or a clip name such as `a00_2007`, `a00_2202`) beside the ghoul's gold skeleton, looping, or held at second `<s>` for a still |
| `WOWSOULS_MOVE=<name>` | the kobold's next attack is this kit move (once some band fits the distance) |
| `WOWSOULS_SEED=<n>` | seed the kobold's dice, for a repeatable fight |
| `WOWSOULS_SETTINGS=<file>` | the settings file (default `%LOCALAPPDATA%\WoWSouls\settings-v1.txt`); an unattended run neither reads nor writes it unless this names one |
| `WOWSOULS_SYSMENU_SCRIPT=<s>:<verb>,...` | the system menu's hands at real second `s`: `open close up down left right confirm back delete capture:<chord> pad:<button>` (`capture:Shift+KeyG`, `pad:North`) |
| `WOWSOULS_MENU=1|0` | show the title and character creation even in an unattended run, or skip them; `WOWSOULS_MENU_SCRIPT=<s>:<press>,...` is their hands (`race:<id> sex:<0|1> class:<slot> dial:<i>:<+1|-1> random create char:<c> up down left right confirm back backspace`), `WOWSOULS_MENU_HOUR=<h|h:mm>` the hour the title's scene is lit at (default dusk, 18:40), `WOWSOULS_CREATE_ALL=1` lets every race be chosen (the others are grey) |
| `WOWSOULS_AUDIO=0|1|null` ||1\|null` | sound: `0` off, `1` the output device even in an unattended run, `null` the whole path with the output discarded and every sound logged; unset: on in an attended run, off in an unattended one |
| `WOWSOULS_VOLUME=<0..1>` | master volume (default 0.6) |
| `WOWSOULS_SHAKE=0` | camera shake off (default on: a blow's jolt scaled by its flinch level, see `shake.rs`) |
| `WOWSOULS_BLOOD=<x>` | blood: `0` off (the gizmo cross returns), unset or `1` the default, a number scales how many droplet emitters a spurt gets (`2` = twice as much; capped at 10). WoW's own melee blood spurt, turned up (see `blood.rs`) |
| `WOWSOULS_POISE=<n>` | the player's poise (default 0, a naked body: every blow flinches, as in Dark Souls); with `n`, blows soak it down by their poise damage and only the one that breaks it flinches (see `reaction.rs`, `hurt.rs`) |
| `WOWSOULS_DSR_CACHE=<dir>` | read the Dark Souls cache from `<dir>` (default `cache/dsr`) |
| `WOW_DATA=<dir>` | the install's `Data` folder |

## Souls and the shops

Souls are the one currency (the Dark Souls counter, bottom right): kills give them, dying leaves them in a
bloodstain, and the valley's vendors sell for them. Press X (pad Y) at a vendor: Janos Hammerknuckle
(weapons), Dermot Johns (cloth and leather), Godric Rothgar (chain and shields), Brother Danil (supplies),
Innkeeper Farley (food). A vendor with a quest lists his quests and "I want to browse your goods". The
shop has two tabs, Buy (the vendor's own stock from the database, in its order) and Sell (the pack, a stack
at a time; what an accepted quest still needs stays); Up / Down choose, Left / Right (LB / RB) switch tab,
Enter / A buys one or sells the stack, Backspace / B leaves. The card on the left shows the highlighted
item with its comparison against what you wear. Prices are WoW's copper at **4 souls to the copper** (a
Cracked Leather Vest 60c = 240 souls, a kobold gives 150); a sale pays WoW's sell price at the same rate;
quest copper is paid out as souls (an older save's copper became souls once). Switches:
`WOWSOULS_SHOP_SCRIPT=<s>:<up|down|left|right|confirm|back>,...`, `WOWSOULS_SOULS_SCRIPT=<s>:give:<n>`.

## Abilities, rage, the trainer and the strongbox

Press X (pad Y) at **Llane Beshere** (warrior trainer, south of the Abbey): the list offers three lessons in the
order they are learned, at WoW's own prices in souls: **Charge** (400, level 1), **Battle Shout** (40, level 1)
and **Thunder Clap** (400, level 4). A warrior knows nothing at the start. **Rage** (the thin red bar under the
stamina bar, the gold one under it is the experience) builds from the blows you deal and take and fades when the
fighting stops. The abilities you know sit on four slots at the bottom left (dim while the rage is short or the
ability is not ready, with the seconds of its cooldown over it): keys 1 to 4 use a slot, Z / left-stick click
cycles the selected one and the D-pad down uses it (the D-pad left is the ranged stance). **Charge** (no rage,
12 s): you rush at the locked enemy, else the nearest ahead, 5 to 25 yd away, gain 9 rage and it stands stunned
for 1.5 s; an attack, a roll or a blow ends the rush. **Battle Shout** (20 rage): your blows do 15 % more for two
minutes. **Thunder Clap** (20 rage, 5 s): every enemy within 5 yd takes 30 damage and moves and recovers 40 %
slower for 10 s. Each plays WoW's own cast animation, spell models and sounds.
**Brother Danil** also keeps the Abbey's strongbox ("I want to use the Abbey's strongbox"): souls banked there
are safe from a death; a deposit pays a tenth to the Abbey, a withdrawal is free; the list offers all, 100 and
1000. Switches: `WOWSOULS_RAGE=<n>`, `WOWSOULS_ABILITY_SCRIPT=<s>:<1-4|cycle|use>,...`.

## Equipment

The paper doll is WoW 1.12's (`wow-world/src/equipment.rs`): head, neck, shoulders, shirt, chest, waist, legs,
feet, wrists, hands, two fingers, two trinkets, back, main hand, off hand, ranged and tabard (the server's
numbers 0 to 18), and the ammo slot the character pane shows under the weapons. The four bag slots are left
out (the pack has no capacity). The G page lays the slots out as the 1.12 pane does and uses WoW's own
empty-slot pictures; Enter on a slot lists what the pack holds for it (a refused item stays listed with its
reason, "Level 4"), Delete takes it off. What goes where follows the item's `inventory_type`; a robe is a
chest piece, a one-hander goes in either hand (the off hand needs Dual Wield: a warrior at level 20), a
two-hander clears the off hand and shuts it, a ring or trinket takes the first free finger / trinket slot.
Required level, class and race masks and the class's armor and weapon types decide who may wear it (a warrior
wears cloth, leather and mail and plate from level 40). WoW's wording is used for the refusals.

What the body shows: armor paints the body atlas and picks geosets (`CharSections` regions, as WoW's own
composite does), the head and shoulders hang their models from the body's attachment points, the back item is
the cloth cloak (`cloak.rs`), the main hand is the held weapon (a bare fist strikes when it is empty), the
off hand a shield on the forearm or a weapon in the left hand. A change dresses the built body again in
place (`actor::build` on a changed `Dress`). Worn items are saved with the pack in `items-v1.txt`
(`equip <slot> <item>` lines; an old save gets WoW's human-warrior starting kit once: Worn Shortsword, Worn
Wooden Shield, Recruit's Shirt, Pants and Boots). Not modelled: weapon skills learned from trainers, bind on
equip, durability, the Show Helm / Show Cloak options, the bag slots; armor and weapon damage do not change
combat yet, the tooltip prints them.

## Dark Souls data

The equipped warrior weapon selects one of 19 Souls profiles, including alternate 1H/2H grips,
fist/unarmed, polearm/staff and bow/crossbow/gun/thrown. Melee uses original DSR animations,
root motion, attack windows, stamina and parameter values retargeted to the WoW human. The roll
is DSR's medium roll (`a00_0710`). Gun/staff and ranged physics contain documented adaptations;
see [weapon sources, exact values and limits](docs/WARRIOR-WEAPONS.md). A missing selected family
disables attacks instead of silently borrowing another weapon's moves. Build from your DSR install:

```powershell
cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- list      # the player's roll clips
cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- extract   # writes cache/dsr/
cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- weapons   # rebuilds all warrior families only
cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- defense   # adds block/parry to an existing cache
cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- loot      # adds contextual opening / pickup clips
cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- locomotion # adds the walk, run, sprint and locked-on steps of each weapon set
cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- loco --set a00 # measures the player's gait clips (read-only)
cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- hud       # writes cache/dsr/hud/ (the HUD)
```

Options: `--dsr <dir>` (default `C:/Spel/Dark Souls Remastered`), `--wow <Data dir>` (default
`C:/Spel/World of Warcraft 1.12/Data`), `--out <dir>` (default `cache/dsr`), `--roll a00_NNNN`.

- Read (never written): `chr/c0000.anibnd.dcx` (the player skeleton and TAE events) and
  `chr/c0000_a*.anibnd.dcx` (the animations), `param/GameParam/GameParam.parambnd.dcx` and
  `paramdef/paramdef.paramdefbnd.dcx` (stamina and damage multipliers) from DSR; the HumanMale
  model from WoW's MPQs.
- Written: `cache/dsr/manifest.txt` (version, the roll, the weapon class with one `move` line per
  move and the blade `grip`, the retarget's measured errors, each source file's size and mtime:
  the game ignores a stale cache) and the roll and the six moves as `.wscl` on the WoW skeleton
  (`human_male/`) and on the DS skeleton (`c0000/`, for `WOWSOULS_DSVIEW`).
- `cache/dsr/locomotion.txt` (`locomotion` command, also the last step of `extract`): the body's
  own Dark Souls gait for each weapon motion set (`a00` one-handed and unarmed, `a10` two-handed
  small weapons, `a12` great weapons, `a13` polearms, `a14` bow, `a16` crossbow): walk, run, sprint
  and the locked-on steps in four directions, blended by speed
  ([`dsloco.rs`](crates/wow-souls/src/dsloco.rs)). Without it WoW's walk and run stay.
  `WOWSOULS_DSVIEW=walk` (or `run`, `sprint`, `lock_run_l`...) shows one beside the DS skeleton.
- The HUD (`hud` command): the layout is `menu/menu.drb.dcx` (Dark Souls' own UI layout, dialogs of
  named objects in a 1280 x 720 space), the art the menu atlases (`menu/menu_N.tpf.dcx`,
  `menu/ENGLISH/menu_local.tpf.dcx`) and the font `font/english/DSFont24`. `cache/dsr/hud/` gets
  one PNG per sprite, two font pages and `manifest.txt`. The game draws the health and stamina bars
  (length = maximum x 3 layout px, a pale chip behind them that drains after a blow), the skull, the
  souls counter (a kill gives the creature's `getSoul`, 150 for the kobold), an enemy bar with name
  and damage number over every creature it has struck, the lock-on ring and YOU DIED from it, scaled
  to the window height (`UiScale` = height / 720). Without it, or with `WOWSOULS_HUD=0`, the plain
  HUD stays.
- `cache/` is gitignored: nothing from the DSR install is committed.
- Defense uses the Shortsword guard params, `a10_0100/0001/0101` raise/hold/lower, `a00_2080..2083`
  guard breaks, and `a42_5100` empty-hand parry. Guard becomes active at 0.20 s in the raise; a held
  guard slows movement and pauses stamina regeneration. Unblockable attacks bypass it. Successful parries
  cancel eligible kobold, thug and Garrick swings and play their own `a00_2100`; wolves cannot be parried.
  The front half-plane and parry timing/overlap rule are prototype choices; the DS engine rule is not in
  the params. Riposte, enemy guard/parry, shield equipment and additive guard-hit clips remain separate work.
- Roll audit (2026-10-09): the cache uses medium roll `a00_0710` (movement cancel 1.10 s), not fat
  roll `a00_0720` (1.60 s). Automatic extraction refuses a missing medium clip instead of selecting a
  different weight class. `--roll` still permits an explicit choice for comparisons.
- Loot: `a00_7521` opens the native WoW chest lid, `a00_7522` takes from an open chest,
  `a00_7520` collects corpse loot. Inventory and pending/claimed chest state save together in
  `%LOCALAPPDATA%\WoWSouls\items-v1.txt`. Three authored supply caches sit on optional detours
  behind the Abbey and at woodland camps; their one-time rewards are usable Abbey Salves.
  The chapel passage is clear, prior claims survive the move, and five native WoW chests keep
  their baseline positions and loot. See [placement report](docs/superpowers/specs/2026-10-09-treasure-placements-built.md).
  Loot/inventory panels use the installed Dark Souls parchment texture and WoW's
  real ItemDisplayInfo icons, with installed Georgia for text. Lootable corpses play WoW's native
  LootFX model, animation and particles until collected.
  The existing combat HUD is unchanged. Quest-only drops await the real quest-log eligibility
  connection; see [loot build report](docs/superpowers/specs/2026-10-09-loot-built.md).
- Licences: the extractor crates `dsr-formats` and `dsr-cache` are GPL-3.0-or-later (ported
  from SoulsFormatsNEXT, SoulsAssetPipeline and HavokLib); the game (`wow-souls`) and the clip
  format (`wscl`) are MIT OR Apache-2.0 and read only `.wscl` files.

## Milestone 1

1. ✅ Workspace and bootstrap: the engine boots from our own binary (free camera)
2. ✅ Spawn hold and stream focus: the world streams around the body, which waits under a loading cover until the ground and its colliders are in, then snaps to the ground
3. ✅ Capsule, kinematic motor (benilla's, ported with its tests), camera-relative controller, a basic orbit camera
4. ✅ Souls third-person camera: boom swept against the world (WMO camera colliders), snap in / ease out, smoothed pivot, recentre, body fade value
5. ✅ Human male model (display 49: geosets, composited skin, hair) on the engine's GPU skinning, with a Stand / Walk / Run / Jump / Fall state machine at gait-matched rates
6. ✅ One hostile kobold (display 365) placed north of the Abbey: notices the player at 20 yd and turns to face them; solid to the body, ignored by the camera. Souls lock-on: camera holds the target, the body faces it and strafes at 4.5 yd/s with backpedal / side-step gaits

## Milestone 2: combat

1. ✅ Weapon in hand: rusty short sword (ItemDisplayInfo 1542) on the right-hand attachment, fingers closed by the `HandsClosed` overlay; F2 draws the blade segment from its trail markers
2. ✅ Combat core: health, stamina (cost 18, regen 45/s after 0.6 s), Souls HUD bars, action state machine with a 0.35 s input buffer, light attack (Attack1H at 1.25x: 0.8 s, active 30-55 %, chain from 70 %, small lunge)
3. ✅ Hits: the weapon (tip marker to hand) swept between frames against capsule hurtboxes, one hit per swing, 20 damage, 0.07 s hit-stop, kobold wound / death clips, enemy bar with a Souls damage number; the lunge stops short of an enemy in reach
4. ✅ Dodge roll, built (1.12 has no roll clip): 4 yd over 0.6 s on a fast-first speed curve through the motor, the model crouched (StealthStand) and turned one forward somersault under the body, i-frames 0.05-0.42 s, 20 stamina; backstep (Dodge clip, 1.8 yd) with the stick still; buffered, cancels a swing's recovery
5. ✅ Enemy AI: the kobold notices within 20 yd, chases on the ground through the motor (4 yd/s), winds up a telegraphed swing (Attack1H at 0.7x, 1.43 s; tracks through the wind-up, committed from the strike at 0.49-0.69 s), 25 damage, 1.2 s between swings, walks home past 30 yd; a hit breaks its swing. The player staggers (0.5 s, CombatWound), dies at 0 (Death, "YOU DIED"), respawns at the spawn point after 4.5 s with the kobold reset; the roll's i-frames let a swing pass through
6. Polish: lock-on line of sight, camera shake, bonfire

Continuing in a new chat: see [docs/HANDOVER.md](docs/HANDOVER.md).

## Known issues

- Pressing into a concave wall corner jitters the capsule by about 3 cm a frame.
- No swimming: water is not solid to the body, which walks along the bottom.
- The body hides abruptly when the camera enters the head; benilla's feathered fade needs the
  blend-twin materials wired.
- The body is naked apart from the sword: no equipment or billboard cards (eye glows) yet.
- The kobold model has no ready stance (ids 25/26), so it stands when alert. It does have attack
  (16, 17) and wound (9, 10) clips for combat.
- Lock-on does not break when a wall comes between the player and the target.
- The player's blade is tested against capsules riding the kobold's bones, fitted to its own
  skinned mesh (F2 shows them in orange); the light attack's arc runs high-right to low-left, so
  spacing and facing decide whether it connects.
- The kobold's swing hits by a reach-and-cone test in its strike window (placed on the clip by
  measuring its hand), not by sweeping its hand; it walks with the player's capsule and passes
  through the player (it stops short by design).
- The locked-on side-step uses WoW's ShuffleLeft/Right clips (11/12), authored for turning in
  place; 1.12 has no strafe animation.
- With the cache, the light attack is Dark Souls' own and slower than the prototype's: R1's hit
  window opens 0.47-0.53 s in, the next move chains from 0.73-0.77 s, an evade from 0.9 s; a kobold
  at 2 yd strikes (0.5 s into its wind-up) before an R1 or R2 can land. The held WoW short sword is
  turned so its blade lies along Dark Souls' (mean 3 degrees off, worst 12); its twist about the
  blade is as WoW authored it (`WOWSOULS_GRIP` tunes it by eye).
- Without the Dark Souls cache the roll is procedural: its crouch is StealthStand, not a tucked
  ball, so the legs swing wide through the somersault. Locked on, the body turns to roll along the
  stick, then back to the target.
