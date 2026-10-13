# Character creation (WoW's own screen, Souls style): design

Date 2026-10-12. Part 1 of 3 of "the menu" (the user: "Gör menyn - jag skulle vilja ha original character
creation från WoW implementerat. Lägg till en wow souls spin på det dock. Sen grafik etc - allt en meny
behöver med keybindings etc"). Parts 2 (settings + key bindings) and 3 (the in-game system menu with
Sound / Graphics / Controls / Game pages) get their own specs when reached.

## Decisions (the user, 2026-10-12)

- **Everything is shown, only the Human Warrior (male) is playable.** All 8 races x 2 sexes x the classes of
  each race can be picked and seen in 3D with every customization; Create accepts only Human Warrior male
  and says why for the rest (the game has no other class kits and no retargeted other bodies).
- **Souls style**: WoW's layout and icons kept, drawn in Dark Souls parchment panels, Georgia / ivory /
  gold, the Create button reads "Prepare to die", the scene is dusk in front of the Abbey.
- No class cards, no starting gift (not chosen).
- **Amended mid-build (the user): only the Human race can be chosen, the other seven are drawn grey** (so the
  "everything can be picked and seen" above holds for sex, class and the dials; `WOWSOULS_CREATE_ALL=1` shows
  every body). **Later: every race gets its own backdrop, its start zone** (see HANDOVER, "LATER, IMPORTANT").
- Nothing is committed (the repo has no commits; the user commits).

## Sources (all read from the user's install at run time, nothing shipped)

- Layout, behaviour, texcoords: `Interface\GlueXML\CharacterCreate.xml` + `.lua` (checked in the install),
  the fork's `benilla-app/src/char_create/{screen,panels,refresh,mod}.rs` as the worked reference
  (`C:\Spel\research\benilla-everwood`, MIT OR Apache-2.0; listed in `docs/PROVENANCE.md`).
- Text: `Interface\GlueXML\GlueStrings.lua` (`RACE_INFO_*`, `ABILITY_INFO_*`, `CLASS_*`, `FACTION_INFO_*`,
  `HAIR_*`, `FACIAL_HAIR_*`, `CHAR_CUSTOMIZATION*_DESC`), parsed without a Lua VM (`KEY = "value";`).
- Data: `benilla_formats::CharCreateCatalog` (already a dependency): `body_display(race, sex)`,
  `classes_for_race`, `ranges(race, sex)` (the five dial counts), `race_file`, `hair_customization`,
  `facial_hair_customization`, `start_outfit(race, class, sex)`.
- Art: `Interface\Glues\CharacterCreate\UI-CharacterCreate-{Races,Classes,Gender,Factions,Banners,IconShadow}`,
  `UI-RotationRight-Big-{Up,Down}`; the Dark Souls parchment from `HudArt` (`soul_param/bg_parchment_0`).
- Sounds: `gsCharacterCreationClass / Look / CreateChar / Cancel` through `Audio::play_named`.

## Screen (1024 x 768 authored units, scaled by window height; the HUD's `UiScale` = height / 720, so a unit is 720/768 layout px)

```
left tower (28,74) 206 x 600        3D character (drag = rotate)         right stack (-20,20) 240 wide
  Alliance | Horde  (banners)                                              Faction   240 x 160
  4 + 4 race icons 48 px (33/127,68)                                       Race      240 x 260 (+ abilities)
  Male | Female (53,303)                                                   Class     240 x 210
  class icons, 3 wide (27,369), only the race's classes
  5 dials (4,480) 198 x 32: Skin, Face, Hair style, Hair colour, Facial hair (hidden when NONE)
  Randomize (30,645)
bottom: Name box (156 x 40, 50 up), rotate left / right (237,0), Create 160x35 and Back 120x30 at (-50,20)
```

Behaviour as the original: a race change resets the class to the race's first, re-clamps the dials and
resets facing to -15 degrees; a sex change re-clamps; the facial-hair dial vanishes for a NONE token; the
info panels scroll by wheel; the name takes letters only (2 to 12), Enter creates, Esc goes back; Randomize
draws every dial in its range. Pad: D-pad moves the focus across icon groups, A selects, B goes back
(rows in the existing `Menu` press model).

## Architecture

- `src/create/` (new): `strings.rs` (GlueStrings parser + resource), `rules.rs` (pure `Rules` + `Selection`,
  fully tested without a window), `art.rs` (handles + texcoord tables), `ui.rs` (spawn, refresh, input),
  `preview.rs` (the 3D body), `mod.rs` (plugin, resources, catalog loading at Startup).
- `menu.rs` keeps the flow state machine (`Menu`, `Press`, `Outcome`, the unattended script) and embeds
  `create::Selection` in its Create screen; its list rendering of the old Create screen goes away, the
  Main (title / select) screen stays a list in the same panel style.
- `character.rs`: `wschar 2` adds `Dials` (skin, face, hair_style, hair_color, facial_hair); a v1 file loads
  with the old default look (skin 2, face 1, hair 3, colour 1, facial 1), so an existing save looks the same.
- `avatar.rs`: the body's look is `WOWSOULS_LOOK`, else the chosen character's dials, else the default;
  when a first character is created the avatar's `ActorModel.look` is replaced and its `Dress` touched
  (the actor builder redoes the parts: the same path the equipment uses).
- `daycycle.rs`: while the menu is up the published `WorldTime` is the menu's dusk (18:40) unless
  `WOWSOULS_HOUR` / `WOW_CLOCK` is set (`WOWSOULS_MENU_HOUR` overrides); the game clock itself is untouched.
- Preview: one `Preview` entity at the avatar's spot, `ActorModel::for_display(body_display, look)`, dressed
  from `start_outfit` (inventory type -> `Dress` slot), idle clip looping. A dial change rewrites
  `ActorModel.look` and touches `Dress`; a race or sex change spawns the next body hidden and swaps when it
  is built (no flash). The avatar is hidden while the screen is up. The camera frames the body by its scale.

## Errors and edge cases

- No client data / catalog fails to load: the screen falls back to the old plain list (Human Warrior).
- Missing art piece: the icon slot shows the label text, as the plain fallback.
- The character file cannot be read: nothing is written over it (existing rule kept).
- A race whose model is absent or still loading: the old body stays until the new one is built.
- Create on a non-playable combination: refused with a message, nothing saved.

## Verification

Unit: class lists per race (from a fixture `Rules`), dial wrap / clamp / randomize range, race and sex
change rules, outfit slot mapping, GlueStrings parsing (fixture + the install file), `wschar` 1 / 2 round
trips, menu flow tests kept. Install tests (skip without `WOW_DATA`): the catalog loads, 8 races x 2 sexes
have bodies and non-zero dial ranges, every `RACE_INFO_*` / `CLASS_*` key resolves. Live (unattended with
`WOWSOULS_MENU=1` + `WOWSOULS_MENU_SCRIPT`): screenshots of Human male default, Orc female, Tauren male,
Night Elf female, Gnome, Troll, a created Human Warrior entering the world with the chosen look. Gates:
scoped fmt `--check`, `cargo clippy -j 4 --workspace --all-targets -- -D warnings`, `cargo test -j 4
--workspace` with `DSR_DIR` / `WOW_DATA`.

## Not in this part

Key bindings, settings file, options pages and the in-game menu (parts 2 and 3); playable other bodies
(needs per-skeleton retarget packs); a per-race glue backdrop model (we use the valley).
