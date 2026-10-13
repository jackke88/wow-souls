# Settings, key bindings and the in-game system menu: design (menu parts 2 and 3)

Date 2026-10-12. The user: "Sen grafik etc - allt en meny behöver med keybindings etc" (after the character creation,
part 1: `2026-10-12-character-creation-design.md`). Approved in the same answer as part 1 ("Kör"): **Esc and the pad's
Menu button open one system menu; Tab / G / L still open their own pages directly.**

## What the player gets

- **Esc** (pad **Start**) opens the system menu in play (when no other screen is up): Resume, Pack, Equipment,
  Quest log, Options, Quit game. The world pauses while it is open (an option). Esc / B closes it.
- **Options** has four pages: **Sound** (master, music, ambience, effects, quest voices, Loop Music), **Graphics**
  (preset Classic .. Ultra and the lanes: shadows, ambient occlusion, bloom, fog, volumetric light, sky, wind, water,
  colour grading, render distance, grass; fullscreen, vsync), **Controls** (every action's key / mouse binding, two
  slots; and the gamepad buttons), **Game** (mouse and stick sensitivity, invert Y, camera shake, the help line,
  pause in the menu).
- A binding is changed by choosing it and pressing the new key (a mouse button, a Shift / Ctrl / Alt chord, or on the
  gamepad page a button); Esc cancels, Delete clears, "Reset to defaults" restores. A key already in use moves
  to the new action and the menu says which action lost it.
- Everything is saved to `%LOCALAPPDATA%\WoWSouls\settings-v1.txt` the moment it changes; the environment switches
  (`WOWSOULS_VOLUME`, `_MUSIC`, `_AMBIENCE`, `_MUSIC_LOOP`, `_GFX` and its lane switches, `_SHAKE`) still win over the
  file for that run, and an unattended run neither reads nor writes it (unless `WOWSOULS_SETTINGS=<file>` names one).

## Architecture

- `src/settings/` (new)
  - `binds.rs` (pure): `Action` (31 actions, ids / labels / groups), `Chord { source: Key | Mouse, mods }`,
    `Binds` (two chords + one pad button per action, `Default` = today's hard-coded keys exactly), evaluation against
    an input snapshot (`Held`): a chord is active when its source is pressed, its modifiers are held, and no more
    specific chord on the same source is also held (so Space rolls, Shift+Space jumps, Ctrl+Space still rolls),
    mouse buttons count only with the cursor captured; `assign` (moves a chord from the action that had it),
    text names (`KeyF`, `Shift+Space`, `Mouse:Left`, `RightTrigger`), the `Controls` system parameter.
  - `mod.rs`: `Settings { sound, video, game, binds }` with the text format `wssettings 1` (`key value` lines, unknown
    keys kept out and logged, values clamped), `VideoPrefs` <-> `VideoConfig`, `initial()` (file + env, read once at
    plugin build), the `Settings` resource, save-on-change (debounced), the apply systems (audio, zone loudness,
    video config, camera, shake, help line, window).
- `src/sysmenu/` (new): `SysMenu` (pure: pages, rows, cursor, capture, `press` -> effects, tested without a window) and
  the Bevy side (open / close, pause, the panel in the title's parchment style, the capture of the next key).
- Call sites of hard-coded keys become `Controls` lookups (controller, camera look, combat, lock-on, weapons, loot
  (pack, equipment, interact, quick use), quests (interact, log, tracker), abilities); the navigation keys of the
  screens (arrows, Enter, Esc, Backspace) and the F-keys of the instruments stay fixed.
- `loot::Screen` gains nothing; `Controls` returns false for every action while the menu is open, so no site needs
  its own gate. The menu's own "was a screen open when this frame began" check keeps the Esc that closes the pack
  from also opening the menu.

## Errors and edge cases

- A settings file that cannot be read or has a wrong header: defaults, the file is not written over until the
  player changes something (then it is replaced); a bad line is skipped and logged, the rest still loads.
- A binding file that leaves an action without any binding is allowed (the player did it); Esc always opens the
  menu, so the player cannot lock themselves out.
- Settings of a lane the build does not carry (no `dev` feature) are plain config values: nothing special.
- The video config applies live; if a lane cannot change a value at run time (a shadow map's size) the value
  takes effect at the next start and the row says so.

## Verification

Unit: binds default parity with the old hard-coded keys (per action), chord matching (Space / Shift+Space /
Ctrl+Space, modifier keys as sources, mouse capture), assign / displace, text round trip for every key in the
table, settings round trip / unknown key / clamp / env override order, VideoPrefs <-> VideoConfig for every rung,
SysMenu navigation (pages, wrap, sliders step and clamp, toggles, capture commit and cancel, reset, effects).
Live (unattended with `WOWSOULS_MENU_SCRIPT`-like `WOWSOULS_SYSMENU_SCRIPT=<s>:<press>,...`): screenshots of the
root, each options page, a binding capture, and the menu over the world; a changed binding visibly changes the
HUD help line. Gates as part 1.

## Not in this part

Per-class bindings, macros, a mouse-wheel binding, chord bindings on the gamepad, a gamma / brightness slider
(the engine has none), resolution choice (the window is the OS's), localisation of the labels.
