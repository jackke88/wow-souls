# Character creation (WoW's screen, Souls style) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline) was chosen: the work is
> one tightly coupled UI and the code is kept in one context. Steps use checkbox syntax.

**Goal:** WoW 1.12's character creation screen (all races, sexes, classes, five dials, Randomize, name, rotate)
in the game, in Dark Souls parchment style, with a live 3D preview; only the Human Warrior (male) is playable.

**Architecture:** `src/create/` holds pure rules (`Selection`, `Rules`), the GlueStrings parser, the art
tables, the Bevy screen and the preview body. `menu.rs` keeps the flow state machine and embeds `Selection`.
`character.rs` gains the dials (`wschar 2`). The spec is
`docs/superpowers/specs/2026-10-12-character-creation-design.md`.

**Tech Stack:** Rust, Bevy 0.18.1 UI, `benilla_formats::CharCreateCatalog`, `benilla_assets` (BLP sprites).

**Standing rules:** nothing is committed (the user commits); `-j 4`; fmt is package-scoped
(`cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats`), never `--all`; gates are
fmt `--check`, `cargo clippy -j 4 --workspace --all-targets -- -D warnings`, `cargo test -j 4 --workspace` with
`DSR_DIR="C:/Spel/Dark Souls Remastered" WOW_DATA="C:/Spel/World of Warcraft 1.12/Data"`.

---

## File map

| File | Role |
|---|---|
| `src/character.rs` (modify) | `Dials`, `Character.dials`, `wschar 2` read/write, v1 default look |
| `src/create/mod.rs` (new) | plugin; `CreateCatalog` + `Strings` resources loaded at Startup |
| `src/create/strings.rs` (new) | `KEY = "value";` parser, `Strings` |
| `src/create/rules.rs` (new) | `Rules` (class lists, dial ranges), `Selection` (pure), `ALLIANCE`/`HORDE`, `Faction` |
| `src/create/art.rs` (new) | glue art paths, sheet sizes, texcoord tables (`race_tc`, `class_tc`) |
| `src/create/ui.rs` (new) | screen tree, refresh, hover / click / wheel, name typing |
| `src/create/preview.rs` (new) | preview body spawn / swap / re-dress, idle, framing |
| `src/menu.rs` (modify) | `Menu` embeds `Selection`; Create screen delegates; flow + script verbs |
| `src/avatar.rs` (modify) | look from the chosen character; `apply_chosen_look` |
| `src/daycycle.rs` (modify) | menu dusk |
| `src/main.rs`, `src/boot.rs` (modify) | `mod create;`, plugin |

---

### Task 1: the saved character carries its look (`wschar 2`)

**Files:** modify `src/character.rs`.

- [ ] **Step 1: tests first** (append to `character::tests`): v1 text loads with `Dials::DEFAULT`; v2 round trip
  with non-default dials; a dial out of `u8` or missing a field is an error; `to_text` writes `wschar 2`.
  The existing round-trip test builds `Character { name, race, class, sex }`: add `dials: Dials::DEFAULT`.
- [ ] **Step 2: run** `cargo test -j 4 -p wow-souls character::` expect compile errors (no `dials`).
- [ ] **Step 3: implement** `pub struct Dials { skin, face, hair_style, hair_color, facial_hair: u8 }` with
  `DEFAULT = Dials { skin: 2, face: 1, hair_style: 3, hair_color: 1, facial_hair: 1 }` (the avatar's old
  `DEFAULT_LOOK`); `Character.dials`; `to_text` writes `wschar 2` + the five keys
  (`skin face hair_style hair_color facial_hair`); `from_text` accepts `wschar 1` (dials default) and
  `wschar 2` (all five keys required).
- [ ] **Step 4: run** the same tests: pass.

### Task 2: GlueStrings

**Files:** create `src/create/strings.rs`, `src/create/mod.rs` (stub with `mod strings;`), modify `src/main.rs`
(`mod create;`).

- [ ] **Step 1: tests** `parses_assignments_and_skips_the_rest`, `escapes_are_unfolded`, plus an install test
  (skips without `WOW_DATA`) that reads `Interface\GlueXML\GlueStrings.lua` through `benilla_formats::open_chain`
  and asserts `RACE_INFO_HUMAN`, `CLASS_WARRIOR`, `FACTION_INFO_ALLIANCE`, `HAIR_NORMAL_STYLE`,
  `CHAR_CUSTOMIZATION1_DESC` resolve and `ABILITY_INFO_HUMAN1` exists.
- [ ] **Step 2: implement** `parse(&str) -> HashMap<String, String>` (the fork's `parse_glue_strings`, adapted:
  `\n`, `\t`, `\"`, `\\`; skip non-matching lines) and `Strings(HashMap)` with `get`, `text(key, fallback)`.
- [ ] **Step 3: run** tests: pass.

### Task 3: the pure rules and the selection

**Files:** create `src/create/rules.rs`; modify `src/create/mod.rs`.

Types (exact):

```rust
pub const ALLIANCE: [u8; 4] = [1, 3, 4, 7]; // Human, Dwarf, Night Elf, Gnome (ChrRaces ids)
pub const HORDE: [u8; 4] = [2, 5, 6, 8];    // Orc, Undead, Tauren, Troll
pub const INITIAL_FACING: f32 = -15.0 * std::f32::consts::PI / 180.0;
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct Ranges { pub dials: [u8; 5] } // skin, face, hair style, hair colour, facial hair
pub struct Rules { classes: HashMap<u8, Vec<u8>>, ranges: HashMap<(u8, u8), [u8; 5]> }
impl Rules {
    pub fn from_catalog(c: &CharCreateCatalog) -> Rules;
    pub fn classes(&self, race: u8) -> &[u8];           // ascending class id; the fallback is every class
    pub fn ranges(&self, race: u8, sex: u8) -> [u8; 5]; // never a 0: max(1)
    #[cfg(test)] pub fn standard() -> Rules;            // 8 races, classes as 1.12, ranges 9/10/11/10/5
}
pub struct Selection { pub race: u8, pub sex: u8, pub class: u8, pub dials: [u8; 5], pub name: String, pub yaw: f32 }
impl Selection {
    pub fn new(rules: &Rules) -> Selection;                // human male, first class, dials 0, yaw INITIAL_FACING
    pub fn pick_race(&mut self, race: u8, rules: &Rules);  // no-op when equal; else class = first, clamp, yaw reset
    pub fn pick_sex(&mut self, sex: u8, rules: &Rules);    // no-op when equal; else clamp
    pub fn pick_class_slot(&mut self, slot: usize, rules: &Rules);
    pub fn cycle_dial(&mut self, dial: usize, dir: i8, rules: &Rules); // wraps within the count
    pub fn randomize(&mut self, rng: &mut u64, rules: &Rules);         // xorshift, every dial in range
    pub fn playable(&self) -> bool;                        // race 1, class 1, sex 0
}
pub fn class_file(class: u8) -> &'static str;              // "WARRIOR" ...
pub fn race_name(race: u8) -> &'static str;                // "Human" ... "Undead" (the client's names)
pub fn class_name(class: u8) -> &'static str;
pub fn faction_of(race: u8) -> Faction;                    // Alliance | Horde
```

- [ ] **Step 1: tests** (in the same file): class list per race comes from rules in ascending id; a race switch
  resets class to the race's first and facing to -15 deg; a sex switch clamps the dials into the new range;
  `cycle_dial` wraps both ways; `randomize` stays in range for every race and sex over 200 draws and is
  deterministic for a seed; `pick_class_slot` ignores a slot past the list; `playable` only for 1/1/0; `ALLIANCE`
  and `HORDE` partition 1..=8.
- [ ] **Step 2: implement** to the signatures above. Randomize uses the fork's xorshift + golden-ratio add.
- [ ] **Step 3: run** `cargo test -j 4 -p wow-souls create::rules` pass.

### Task 4: the catalog at Startup

**Files:** modify `src/create/mod.rs`, `src/boot.rs`.

- [ ] **Step 1: install test** (skips without `WOW_DATA`): `CharCreateCatalog::load` on the real chain; for each of
  the 8 races and both sexes `body_display` is `Some`, `ranges` is `Some` with every count >= 1, and
  `Rules::from_catalog` gives Human the classes 1,2,3,4,5,8,9 and Orc 1,3,4,7,9 (1.12's combos).
- [ ] **Step 2: implement** `CreateCatalog(Arc<CharCreateCatalog>)` + `Rules` + `Strings` resources inserted by
  a Startup system `.after(AssetSet::Open)` from `WorldAssets.chain` (warn and insert nothing on error);
  `pub fn plugin(app)` registered in `boot.rs` before `menu::plugin`.
- [ ] **Step 3: run** the tests: pass.

### Task 5: art tables

**Files:** create `src/create/art.rs`.

- [ ] **Step 1: tests**: `race_tc(race, sex)` per `RACE_ICON_TCOORDS` (human male `[0,.25,0,.25]`,
  orc male `[.75,1,.25,.5]`, scourge female `[.25,.5,.75,1]`); `class_tc` per `CLASS_ICON_TCOORDS` (mage
  `[.25,.49609375,0,.25]`); `sheet_size` of each sheet matches the BLP header in the install (skips without data).
- [ ] **Step 2: implement** `Sheet { Races, Classes, Gender, Factions }` with path and pixel size
  (256x256, 256x256, 128x64, 128x64), `rect(sheet, [l, r, t, b]) -> Rect`, `load(&AssetServer, path)`
  through `loot::ui::load_icon`-style `texture_url(.., (false, false))` with the sprite variant.

### Task 6: the menu embeds the selection

**Files:** modify `src/menu.rs`.

- [ ] **Step 1: tests**: keep the existing flow tests (adapt the Create-screen ones to the new rows); add:
  Create on a non-playable combination returns `None` and sets the "Only a Human Warrior can be made yet."
  message; Create on Human Warrior returns `Outcome::Create` with the selection's dials; the script verbs
  `race:<n>`, `sex:<n>`, `class:<slot>`, `dial:<i>:<+1|-1>`, `random`, `name:<text>` parse and apply.
- [ ] **Step 2: implement**: `Menu` gets `sel: create::Selection` and `rules: Arc<Rules>`; `Screen::Create`
  rows become the focus ring `Race, Class, Sex, Dial0..4, Randomize, Name, Create, Back` for keyboard / pad
  (Up / Down move, Left / Right change the focused row, Enter acts, Esc / B backs); mouse is the new UI's
  own. `Outcome::Create(Character)` carries `dials`. Old list rendering of Create is removed; Main stays.

### Task 7: the screen

**Files:** create `src/create/ui.rs`; modify `src/menu.rs` (spawn / despawn hooks).

- [ ] **Step 1: implement** the tree exactly as in the spec's layout table (unit `K = 720.0 / 768.0` layout px):
  left tower (parchment 9-slice, banners, 2 x 4 race buttons, gender pair, class grid, five dial rows with
  `<` `>` buttons, Randomize), right stack (three parchment panels with a header icon, title, scrolling body,
  the race's abilities in gold), bottom (name box, rotate left / right, "Prepare to die", Back). Georgia
  font, `loot::ui` palette. Components `Act(CreateAction)`, `Dyn*` markers for refresh.
- [ ] **Step 2: refresh** (on `Selection` change): icon rects from `race_tc` / `class_tc`, selected = gold edge,
  dial labels from Strings (`HAIR_<tok>_STYLE` ...), info panels from `RACE_INFO_<FILE>`,
  `ABILITY_INFO_<FILE><n>`, `CLASS_<FILE>`, `FACTION_INFO_*`, class slots collapse past the race's classes,
  facial-hair row hidden for `NONE`, panel tint by faction.
- [ ] **Step 3: input**: clicks (release over the same button), name typing letters only (2..12), wheel scroll on
  the hovered panel, hold-to-rotate buttons and drag on the 3D area, the sounds
  (`gsCharacterCreationClass` / `Look` / `CreateChar` / `Cancel`).

### Task 8: the preview body

**Files:** create `src/create/preview.rs`.

- [ ] **Step 1: tests** (pure): `outfit_dress(items) -> [u32; 10]` maps inventory type to the `Dress` slot
  (head 1, shoulder 3, shirt 4, chest 5 / 20, belt 6, legs 7, feet 8, wrist 9, hands 10, tabard 19) and the
  cloak (16); a weapon or shield type maps to nothing; `framing(scale) -> (distance, look_height, cam_height)`.
- [ ] **Step 2: implement** the entity, the swap-when-built, the dial re-dress (`ActorModel.look` + touch
  `Dress`), idle (`ids::STAND` looping through `AnimationTransitions`), yaw from `Selection.yaw`, avatar hidden
  and camera set in `Phase::Menu` on the Create screen.

### Task 9: the avatar's look, dusk

**Files:** modify `src/avatar.rs`, `src/daycycle.rs`.

- [ ] **Step 1: tests**: `look_for(chosen: Option<&Character>, env: Option<CharLook>)` picks env, else the
  character's dials, else `DEFAULT_LOOK`; the daycycle `menu_minute(env_hour, pinned, unattended)` returns
  18:40 only when nothing else names the hour.
- [ ] **Step 2: implement**; `apply_chosen_look` on `OnEnter(Phase::Playing)` replaces `ActorModel.look` and
  touches `Dress` when `Chosen` changed after the avatar was built; `publish` reads the phase.

### Task 10: verify and document

- [ ] Gates (fmt `--check`, clippy `-D warnings`, workspace tests with the installs set).
- [ ] Live, unattended with `WOWSOULS_MENU=1`, `WOWSOULS_MENU_SCRIPT`, `WOWSOULS_SHOT*`: Human male default,
  Orc female, Tauren male, Night Elf female, Gnome male, Troll female, dial changes, a Human Warrior created
  and entering the world with its look; pictures to `docs/evidence/create-*.png`.
- [ ] Update `docs/HANDOVER.md` (START HERE job 2 becomes "menu part 1 built; parts 2 and 3 next"),
  `README.md` controls, `docs/PROVENANCE.md` (the screen's layout from the fork's `char_create`), memory.

---

## Self-review

Spec coverage: layout (T7), behaviour (T3, T7), data (T4), strings (T2), art (T5), preview (T8), save (T1),
look to the avatar (T9), dusk (T9), fallback without data (T6 keeps the list when `Rules` is absent),
verification (T10). Types: `Selection`, `Rules`, `Dials`, `CreateAction` are named once above and reused.
