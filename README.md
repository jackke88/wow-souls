# wow-souls

A single-player, third-person souls-like action prototype in the World of Warcraft 1.12.1 world
(Northshire, Elwynn Forest, Westfall), built on [benilla](https://github.com/samwhosung/benilla)'s
world engine (Rust + Bevy). It runs with no server, no login, no network and no WoW UI, and reads
the game's data from your own 1.12.1 install without writing to it.

> **Fan project, prototype quality.** Not affiliated with or endorsed by Blizzard Entertainment or
> FromSoftware. No game files from either company are in this repository: you need your own
> World of Warcraft 1.12.1 client and your own Dark Souls Remastered.

## Play

**You need**

- Windows 10/11 64-bit and a GPU with Vulkan or DirectX 12.
- Your own **World of Warcraft 1.12.1 (build 5875)** client: the folder with `WoW.exe` and `Data\`.
- Your own **Dark Souls Remastered** (Steam): the combat animations are built from it on the first start. They are the heart of the game.
- [Rust](https://rustup.rs) (the right toolchain is picked automatically from `rust-toolchain.toml`).

**Build and start:** clone or download the repository and double-click **`Play.bat`**. It builds the
game the first time (10-20 minutes) and starts it. Or from a terminal:

```powershell
git clone <this repository> wow-souls
cd wow-souls
.\Play.bat
```

**First start**

1. A folder dialog asks for your **WoW 1.12.1 folder** (the folder holding `WoW.exe`, or its `Data`
   folder). The game checks that it is a 1.12.1 client and remembers it in
   `%LOCALAPPDATA%\WoWSouls\wow-folder.txt`.
2. The game asks you to **build the combat animations** from your **Dark Souls Remastered** folder
   (in Steam: right-click the game, Manage, Browse local files). Answer Yes. It takes about half a
   minute, reads your game folder without changing it, and checks every one of the 1,571 clips against
   the developer's copy. The result goes to `%LOCALAPPDATA%\WoWSouls\anim`. These animations are
   what the game is built around: every move of the player and of every creature comes from them.

Neither game folder is ever written to, and nothing is downloaded.

- **Pick another WoW folder:** `Play.bat --choose-wow`, or delete `wow-folder.txt`. If the
  remembered folder disappears, the dialog opens again by itself.
- **Build the animations later / again:** `Play.bat --setup-animations`.
- **Override:** `WOW_DATA=<path to Data>` wins over the remembered folder; `WOWSOULS_ANIM_DIR` over
  the animation folder.
- **Saves and settings:** `%LOCALAPPDATA%\WoWSouls\`.

To play without the source afterwards, put `target\ship\wow-souls.exe`,
`crates\dsr-cache\target\release\wow-souls-dsr.exe`, `data\` and `anim\` in one folder and start the
exe. Original Dark Souls (Prepare to Die Edition) is not supported: its files use older formats.

## About the code

Design and architecture: [docs/wow-souls-feasibility.md](docs/wow-souls-feasibility.md).
What is copied from benilla, and the pinned commit: [docs/PROVENANCE.md](docs/PROVENANCE.md).
Code is MIT OR Apache-2.0 ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)); the
animation builder `crates/dsr-cache` (`wow-souls-dsr.exe`) and `crates/dsr-formats` are
GPL-3.0-or-later and run as a separate program, never linked into the game. Quest data and voices: see [data/README.md](data/README.md).

Source-to-original workflow: [docs/SOURCE-TO-ORIGINAL.md](docs/SOURCE-TO-ORIGINAL.md). Combat
feel was first prototyped against Dark Souls Remastered / Elden Ring reference material on the
developer's machine; that material stays local (`.gitignore`), and the goal is our own replacements.
See [the dependency audit](docs/log/2026-10-17-wow-only-dependencies.md).

## Layout

The repository includes `crates/` (game and data code), `vendor/benilla/` (the complete engine
source dependency closure and licences) and `data/world/` (active world/quest exports). No sibling
engine checkout or database is needed to build/play. Normal open-source libraries are resolved
through the pinned Cargo.lock. Your WoW 1.12.1 installation stays outside the repository and is
read only. `anim/source/` holds the animation tables; its `.wsanim` clips exist only on the developer's
machine or are built locally by `wow-souls-dsr setup` from `anim/provenance/clip-sources.tsv`.

## Run (development)

```powershell
cargo run -j 4 -p wow-souls
```

The dev build finds the install through the `WoW` junction in the project folder, else the
remembered folder or the dialog. `WOW_DATA=<path to Data>` overrides it.

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
| Hold E / pad LB | weapon block with its damage cut and stability; protects the front and costs stamina on impact |
| H | toggle 1H/2H grip for dagger, sword, axe or mace; large two-handed weapons stay two-handed |
| Y / pad D-pad left | draw / put away the equipped ranged weapon |
| C / pad LT | parry with the empty left hand; costs 14 stamina, active for 7 frames at 30 fps; a miss has recovery and no i-frames |
| X / pad Y | open a nearby chest, then press again to take its contents; collect corpse loot |
| Tab (pad: Esc / Start -> Pack, or bind a button) | open / close inventory; movement and combat input are held while it is open; damage closes it |
| Up / Down / W / S / pad D-pad | select an inventory item; mouse clicks also select rows |
| Enter / pad A / Use button | use the selected Abbey Salve; on a wearable item, equip it (WoW's quick equip: the first free legal slot) |
| G / pad Select | the equipment page: WoW's character pane, all 19 equipment slots and the ammo slot (see Equipment below); G again closes it, Tab goes to the pack |
| M / Menu → Map | area map: wheel / +/- / pad L2-R2 zoom, drag / arrows / left stick pan, click / Enter / A place a saved marker, Delete / X remove, R / Y find player, Esc / B close |
| Up / Down, Left / Right (or [ ]), pad D-pad, pad LB / RB | on a quest offer, hand-in or in the quest log: look at the reward items (an icon each); the card on the left shows the item's tooltip and how it compares with what you wear in that slot. With several choices, Enter / A picks and a second press hands in |
| V / pad X | quick-use an Abbey Salve while alive, idle and hurt (restores up to 30 health) |
| Escape / pad B / Close button | close inventory (on the equipment page: first leaves a slot's list) |
| Space / pad B | roll along the stick; with the stick still, backstep; a light press just after it is the rolling attack |
| Shift+Space / pad A | jump |
| Mouse / arrow keys / right stick | orbit the camera |
| Q / middle mouse / right-stick click | lock on to the enemy in front (20 yd); press again to let go; with nothing in reach, recentre the camera |
| Clear horizontal mouse swipe / right-stick flick / look arrow | while locked, switch to the next enemy on that side; let the input rest before another switch |
| Esc / pad Start | the menu: Resume, Pack, Equipment, Quest log, Map, Options (Sound, Graphics, Controls, Gamepad, Game), Quit game; the world stands still while it is open (an option), Esc / B steps back |
| F1 | toggle the free-fly debug camera (hold right mouse to look, WASD, Space/C, Shift) |
| F2 | draw the collision capsule and the blade segment |
| F3 | hide the dev readout and the help line |
| F6 | every creature home again, whole and calm (to fight them again without dying first) |

Lock-on ignores small, slow and mainly vertical mouse movements. One continuous sweep or held stick produces one target change; a short neutral pause rearms it. Mouse orbit sensitivity does not change that gesture. The locked camera eases between targets; free orbit responds directly.

The native HUD's top-center compass follows the camera and shows **?** for completed quests that can be handed in. It shows the destination NPC and distance, groups nearby bearings and uses edge arrows for goals behind the camera. A marker disappears after handing in or abandoning the quest; the original overhead NPC signs are preserved.

First visits show a centered, softly fading area title: Elwynn Forest just outside Northshire's gates, Goldshire, Stormwind City and other main zones. Discoveries are saved per character; menus and dialogue defer the title. The embedded [EB Garamond](crates/wow-souls/assets/fonts/eb-garamond/SOURCE.txt) font includes its [SIL Open Font License](crates/wow-souls/assets/fonts/eb-garamond/OFL.txt), which must accompany distributions.

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
| `WOWSOULS_OBSERVE[=x,y,z]` | render from an observer offset (default overhead) with the camera rig drawn; `WOWSOULS_OBSERVE_HEIGHT=<yd>` raises its inspection target |
| `WOWSOULS_VILLAGE=0` / `=1` | disable / explicitly enable six background road travellers around Goldshire; enabled with the packaged world; original scenery and residents stay unchanged |
| `WOWSOULS_VILLAGE_TRACE=1` | log traveller routes, actual/expected locations and physical walking progress |
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
| `WOWSOULS_LOCK_SWITCH=<s>:<left\|right>,...` | direct target changes for camera checks |
| `WOWSOULS_LOCK_INPUT=<s>:mouse:<dx>:<dy>:<duration>,...` | unattended raw-pixel gesture replay through the real detector; `s:stick:push:duration` replays a stick/key hold |
| `WOWSOULS_LOCK_SCENE=crowd\|wide\|near\|moving\|actors` | unattended diagnostic targets; `actors` uses stationary NPC entries 197/823/196/152 from an isolated manifest |
| `WOWSOULS_LOCK_TRACE=1` | log unshaken view/boom angular speeds against the camera's presentation clock |
| `WOWSOULS_KOBOLD_AT=x,y[,z]` | move the first kobold (WoW coords: +x north, **+y west**); the others stand at fixed offsets from it |
| `WOWSOULS_KOBOLDS=<n>` | how many kobolds (0 to 4; default 4 for a person playing, 1 in an unattended run or when `WOWSOULS_KOBOLD_AT` is set, so the scripted checks keep a lone kobold) |
| `WOWSOULS_RESET=<s>` | press the F6 reset once at `s` seconds |
| `WOWSOULS_HOGGERS=<n>` / `WOWSOULS_HOGGER_AT=x,y[,z]` | Hogger in the slice at `HOGGER_AT` (WoW coords; off unless one of these is set; in the manifest world he stands at his real place, Riverpark, with no switch). Unattended runs need `WOWSOULS_WORLD=1` for the manifest world |
| `WOWSOULS_BOSS_HP=<0.05..1>` | a boss starts with that share of its health (phase two at once below 0.5) |
| `WOWSOULS_BOSS_MUSIC=<path>` | one track (a path in the WoW client's data, e.g. `Sound\Music\ZoneMusic\Naxxramas\NaxxramasKelThuzad1.mp3`) as the boss theme instead of Hogger's list |
| `wow-souls --trial-character <level> [slot]` | write a level 6-12 warrior with the 1.12 green gear set into a new save slot (never over a character) and exit |
| `WOWSOULS_SHOT_DEATH=<s>[,<s>...]` | with `WOWSOULS_SHOT`: write the frame `s` seconds after the player's first death instead of at a fixed time (several: `<name>-<s>.png` each), for the YOU DIED banner |
| `WOWSOULS_HUD=0` | plain development HUD instead of the native Elden Ring reference UI |
| `WOWSOULS_UI=legacy`, `WOWSOULS_VITALS=legacy`, `WOWSOULS_DSR_CACHE` | retired and ignored; cannot enable DSR assets |
| `WOWSOULS_COMPASS=0` | hide the native HUD compass (default on with the native UI); ready quests otherwise show return bearings |
| `WOWSOULS_COMPASS_TRACE=1` | compass diagnostics once per real second: camera heading, target NPC, relative bearing, edge arrow and distance |
| `WOWSOULS_AREA_TITLE=0` | hide first-visit area titles (default on with the native UI); discoveries live in each character's `areas-v1.txt` |
| `WOWSOULS_AREAS_SAVE=<file>` | override the discovery journal; unattended checks use an isolated cache file by default |
| `WOWSOULS_AREA_TRACE=1` | log actual player area changes, resolved title identity and WoW coordinates |
| `WOWSOULS_HUD_GAIN=<x>` | multiply the colour of the HUD's art (Dark Souls' bars are dark; default 1.0) |
| `WOWSOULS_SHOW_BODY=1` | start with the F2 overlay on |
| `WOWSOULS_LOOK=s,f,h,c,b` | the human male's skin, face, hair style, hair colour, facial hair (default `2,1,3,1,1`) |
| `WOWSOULS_RIG_VIEW=<clip>` | side-by-side: a cached Dark Souls clip (`1` is the roll, `straight_sword_light_1` or `r1_1` a sword move, `walk` / `run` / `sprint` / `lock_run_l`... a gait clip) on its own skeleton (gold gizmo bones and a pale blade, 1.5 yd to the avatar's right) beside the human playing the retargeted clip, looping in place; a magenta line lays Dark Souls' blade axis over the WoW sword; logs `rig_view t=` 10×/s and, per loop, `blade error` (the WoW blade against Dark Souls') |
| `WOWSOULS_GRIP=rx,ry,rz` | rotate the held sword (degrees about the hand's axes) while Dark Souls clips pose the body, instead of the automatic fit onto Dark Souls' blade axis |
| `WOWSOULS_RIG_VIEW=kobold:<clip>[@<s>]` | the kobold plays a kit clip (`small_1`, `big`, `run_strike`, `body_check`, `pounce`, `small_2` or a clip name such as `body_reaction_08`, `body_death_03`) beside the ghoul's gold skeleton, looping, or held at second `<s>` for a still |
| `WOWSOULS_MOVE=<name>` | the kobold's next attack is this kit move (once some band fits the distance) |
| `WOWSOULS_MOTION=1` | experimental DSR-derived remix of every animation (default off): enemy attacks get a held wind-up, shorter hit windows and longer recovery (specs rebuilt from the clip); every attack, enemy and player, gets a trunk coil read from the hand's swing, zero inside the hit windows so the blade path is the DS clip's; neck/head/jaw/tail/ears become springs on flinches, deaths, guards, casts, evades, gaits and the stance, and at runtime on enemies' own WoW animations. Player timing, legs and arms are the DS clips. Editable constants: `crates/wow-souls/src/remix.rs`; report: `docs/log/2026-10-15-motion-remix.md` |
| `WOWSOULS_WOLF_MOTION=1` | experimental DSR-derived wolf remix (default off): new bite/lunge rhythm and authored neck/head/jaw/tail curves; pose, root motion and hit windows stay synchronized. Applies to the Wolf family and Prowler's bite; source clips stay intact. Editable recipes: `crates/wow-souls/src/wolf_motion.rs`; report: `docs/log/2026-10-15-wolf-motion-experiment.md` |
| `WOWSOULS_SEED=<n>` | seed the kobold's dice, for a repeatable fight |
| `WOWSOULS_SLOT=<n>` | play character slot `n` (`%LOCALAPPDATA%\WoWSouls\characters\<n>\`: the six saves of one character); without it the slot played last (`active-v1.txt`), else the first character. The title lists the characters (up to 10); picking another relaunches the game on it with this switch, Delete / the pad's X archives one to `backup-<stamp>-character-<n>`. The loose saves of the single-character game move into slot 1 the first time. An unattended run never relaunches |
| `WOWSOULS_SETTINGS=<file>` | the settings file (default `%LOCALAPPDATA%\WoWSouls\settings-v1.txt`); an unattended run neither reads nor writes it unless this names one |
| `WOWSOULS_SYSMENU_SCRIPT=<s>:<verb>,...` | the system menu's hands at real second `s`: `open close up down left right confirm back delete capture:<chord> pad:<button>` (`capture:Shift+KeyG`, `pad:North`) |
| `WOWSOULS_MAP_SCRIPT=<s>:<verb>,...` | map checks: `open close zoom:<steps> pan:<u>:<v> point:<u>:<v> mark clear center`; times are real seconds, coordinates are normalized map positions |
| `WOWSOULS_MAP_SAVE=<file>` | override the per-character `map-v1.txt` marker journal; unattended checks default to an isolated cache file |
| `WOWSOULS_MAP_AREA=<id>` | load another installed AreaTable map (default 12, Elwynn Forest); unavailable charts show a closable error |
| `WOWSOULS_MENU=1|0` | show the title and character creation even in an unattended run, or skip them; `WOWSOULS_MENU_SCRIPT=<s>:<press>,...` is their hands (`race:<id> sex:<0|1> class:<slot> dial:<i>:<+1|-1> random create char:<c> up down left right confirm back backspace`), `WOWSOULS_MENU_HOUR=<h|h:mm>` the hour the title's scene is lit at (default dusk, 18:40), `WOWSOULS_CREATE_ALL=1` lets every race be chosen (the others are grey) |
| `WOWSOULS_AUDIO=0|1|null` ||1\|null` | sound: `0` off, `1` the output device even in an unattended run, `null` the whole path with the output discarded and every sound logged; unset: on in an attended run, off in an unattended one |
| `WOWSOULS_VOLUME=<0..1>` | master volume (default 0.6) |
| `WOWSOULS_SHAKE=0` | camera shake off (default on: a blow's jolt scaled by its flinch level, see `shake.rs`) |
| `WOWSOULS_BLOOD=<x>` | blood: `0` off (the gizmo cross returns), unset or `1` the default, a number scales how many droplet emitters a spurt gets (`2` = twice as much; capped at 10). WoW's own melee blood spurt, turned up (see `blood.rs`) |
| `WOWSOULS_POISE=<n>` | the player's poise (default 0, a naked body: every blow flinches, as in Dark Souls); with `n`, blows soak it down by their poise damage and only the one that breaks it flinches (see `reaction.rs`, `hurt.rs`) |
| `WOWSOULS_MANA=<n>` | a mage starts with `<n>` mana (at most its maximum; default full) |
| `WOWSOULS_ANIM_DIR=<dir>` | editable animation library (default `anim/source`); see [animation guide](anim/README.md) |
| `WOWSOULS_DATA_DIR=<root>` | package root containing anim/source and data/world; otherwise data is found beside the executable or in the current checkout |
| `WOWSOULS_BIND_SAVE=<file>` | override this character's `soulbind-v1.txt` graveyard binding; unattended runs otherwise use an isolated file |
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

At a graveyard's **memorial stone**, press Interact (default X / pad Y) to **Bind Soul**. Your character
then returns beside that stone after death and when starting again. A new binding replaces the previous
one and is saved per character; binding is free and unavailable during combat. The carried souls you
drop on death still wait at the death spot. Falkhaan Isenstrider, who offers *Rest and Relaxation*, also
offers *Bind Your Soul*: bind at Northshire's memorial, then return to him. A previously saved binding
counts too. The stones use the database's graveyards in Northshire, near Goldshire and at Eastvale.

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

**The Mage** (the second class in the creation menu) has **mana** instead of rage: the blue bar under the
stamina bar, 165 at level 1 from the class tables and Intellect, coming back with Spirit after five seconds
without a cast (a level-up or a piece of gear lengthens the bar, it never refills it). The bar, the keys 1 to 4
and the cycle work as above. Known from the start: **Fireball** (30 mana, 14 to 22 fire damage and a burn of
2 over 4 s, 35 yd) and **Frost Armor** (60 mana, 30 armor for half an hour). **Khelden Bremen** in the Abbey
teaches **Frostbolt** (400 souls, level 4: 18 to 20 frost damage, 40 % slower for 5 s, 30 yd) and
**Fire Blast** (level 6: 24 to 32 fire damage, 20 yd, 8 s cooldown). A cast is a heavy two-handed move of
Dark Souls', as Elden Ring's sorceries are (the staff raised, then swung or thrust with the whole body, a
step forward), and the spell leaves the staff where the weapon connects: Fireball is the greatsword's
two-handed R1, a lunging diagonal slash (the ball leaves the tip 0.6 s in, you may act again 1.2 s in of
2.5 s), Frostbolt the staff's own R1 thrust (0.53 s, free from 1.0 s), Fire Blast the two-handed mace's R2,
the staff raised overhead and slammed down (0.93 s, free from 1.7 s), Frost Armor the pyromancy buff gesture
(the staff raised overhead; the body stays put). The body is committed from the press until the clip's
recovery begins (from there a roll, an attack, another spell or a push of the stick leaves it). The missile
steers a little (WoW's speed and range, Dark Souls' homing), and a blow that staggers you in the wind-up
cancels the cast with its mana spent. The wind-up's flames in the hands and its sound end with the release
(or with whatever breaks the cast); the release is a short flash. A hit lands with Dark Souls' own impact for the spell: Fireball bursts 1 m wide (it
takes in whoever stands within that of the burst), flinches heavily (level 3) and pushes 1.2 m; Fire Blast
flinches at level 2 and pushes 0.8 m; Frostbolt is a light flinch with a 0.7 m push. A missile goes at the
locked enemy, else straight ahead; Fire Blast takes the locked enemy or the nearest one in front within its
range, and nothing is paid when there is none. Enemy damage is not scaled: a level 1 mage has 52 health.
Switch: `WOWSOULS_MANA=<n>` starts a mage with that much mana.

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

## Animations

All 19 weapon families, player locomotion/roll/defense/reactions/casts/interactions and creature
kits load from editable `.wsanim` files and tables in [`anim/source/`](anim/README.md), built on
your machine from your own Dark Souls Remastered. Missing or malformed text fails visibly; binary
`.wscl` caches cannot replace it. The animation builder, `crates/dsr-cache` (`wow-souls-dsr.exe`),
and `crates/dsr-formats` are GPL-3.0-or-later and run as a separate program on the first start
(or with `Play.bat --setup-animations`), never linked into the game. To rebuild the tables from
the source clips:

```powershell
cargo run -j 4 --manifest-path crates/dsr-cache/Cargo.toml -- bake-project --from cache/dsr --out anim/source
```

The UI uses native Bevy layout, embedded project art and live text. `WOWSOULS_HUD=0` retains plain
meters; F3 toggles dev text.

Checks before a change:

```powershell
cargo fmt -p wow-souls -p wow-world -p wscl -- --check
cargo test -j 4 --workspace --offline
cargo clippy -j 4 --workspace --all-targets --offline -- -D warnings
cargo build -j 4 -p wow-souls --no-default-features --offline
```

Where the animations are headed: [SOURCE-TO-ORIGINAL.md](docs/SOURCE-TO-ORIGINAL.md).

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
- Locked on, the body turns to roll along the stick, then back to the target.

## Credits and legal

WoW Souls is built on [benilla](https://github.com/samwhosung/benilla) by its authors, which does
all the heavy lifting: every WoW file format, the renderer, the animation and spell systems and
the interface engine. What is taken from it, and the pinned commit, is in
[docs/PROVENANCE.md](docs/PROVENANCE.md); the techniques benilla itself ports from other projects
(WarcraftXL and others) are credited in [vendor/benilla/THIRD-PARTY.md](vendor/benilla/THIRD-PARTY.md).

This is an independent fan project, not affiliated with or endorsed by Blizzard Entertainment or
FromSoftware. It ships no client files from either company: no Blizzard art, models, sounds, maps,
MPQ contents or UI files, and no FromSoftware animations or other assets. You provide your own
legally obtained World of Warcraft 1.12.1 client and your own Dark Souls Remastered; the combat
animations are built on your machine from your copy, by a separate tool, on the first start. The
quest, spawn and item tables in `data/world/` are vMaNGOS exports and the quest voice lines are
Spoken Player recordings; their provenance and terms are in [data/README.md](data/README.md) and
`data/voices/README.md`. World of Warcraft and Warcraft are trademarks of Blizzard Entertainment,
Inc. Dark Souls is a trademark of FromSoftware, Inc. and Bandai Namco Entertainment Inc.

The code is licensed under MIT or Apache-2.0, at your option, as benilla is
([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)). The animation builder
`crates/dsr-cache` and `crates/dsr-formats` are GPL-3.0-or-later and run as a separate program,
never linked into the game. The components vendored under `vendor/benilla/` and the embedded
[EB Garamond](crates/wow-souls/assets/fonts/eb-garamond/SOURCE.txt) font keep their own upstream
licenses, alongside each.
