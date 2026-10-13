# Gear Effects Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** What the player wears changes a fight: armor lowers damage taken (WoW's formula), a weapon's WoW damage per second scales the Dark Souls moveset's damage, a shield's block value shrinks a blocked blow.

**Architecture:** One pure rules module in `wow-world` (`gear_effect.rs`: `GearEffect` computed from `Gear` + items, with `taken()`), one tiny game module (`src/gear_effect.rs`: the `GearFx` resource kept current from `Owned` + `Data`, the `WOWSOULS_GEAR_EFFECT=0` switch), and four one-line hooks where damage is produced (`hits.rs` melee, `projectiles.rs` ranged, two `PlayerHurt` writers in `npc.rs`). Armor and block are applied where `PlayerHurt.damage` is made, so the shake, blood, voice and HUD readers all see the reduced number.

**Tech Stack:** Rust, Bevy 0.18.1 (`wow-souls` crate), pure `wow-world` crate.

**Spec:** `docs/superpowers/specs/2026-10-12-gear-effects-design.md` (approved 2026-10-12). Three implementation choices differ from the spec's text and the spec was updated to match: (1) armor and block are applied at the two `PlayerHurt` writers in `npc.rs`, not in `hurt.rs`, so `hurt.rs` and `defense.rs` are not edited at all; (2) block is a multiplier on the blocked blow's chip damage (`1 - 0.05 x block`, at most 0.5 off), not an addition to the kit's `cut`; (3) the display change is the equipment page's totals line only, WoW's tooltip text is untouched.

## Global Constraints

- **No git commits.** The repo has no commits and the user has not asked for one: every "commit" step of the usual template is replaced by running the gates. Do not run `git add` / `git commit`.
- Build and test with `-j 4` (machine is RAM-tight). Never run bare `cargo fmt --all`; the fmt gate is `cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check` (fix with the same without `--check`).
- Gates at the end of every task that changes Rust: that fmt check, `cargo clippy -j 4 --workspace --all-targets -- -D warnings`, and the task's tests. Final task runs the whole workspace tests with `DSR_DIR="C:/Spel/Dark Souls Remastered" WOW_DATA="C:/Spel/World of Warcraft 1.12/Data"` set in the same shell command.
- `hits.rs` and `projectiles.rs` are co-owned with another chat (weapons / animation): re-read the file immediately before editing, keep the edit to the lines listed.
- No new hardcoded install path (`C:\Spel\...`) in code; the ones in this plan are only in shell commands for a developer.
- Python hangs the Bash tool in this environment: use the Edit tool, `perl`, `sed` or `awk` for scripted edits.
- Bevy trap: a system with more than 16 parameters does not compile as a system; group parameters in a tuple if a hook would pass 16 (neither hook below is near).
- Constants (from the spec's "Tuning knobs"): armor `armor / (armor + 400 + 85 x level)` capped at 0.75; `REF_DPS` = Worn Shortsword = (1 + 3) / 2 / 1.9 s; weapon exponent 0.5, factor clamp 0.75 to 2.5; block 0.05 per point, at most 0.5 off the chip; equipment page quotes armor at attacker level 3.

## Review Focus

The inputs the spec implies but does not spell out, most likely first. Each has a test in the task named.

1. **Empty hands / empty gear:** no weapon in the slot, nothing worn: every factor 1.0, reduction 0, no NaN (Task 1).
2. **A weapon row with no swing time or no damage** (a data hole, a wand, a "Monster - Item"): factor 1.0, never 0 and never NaN (Task 1).
3. **Gear naming an item the table lacks** (an old save whose item left the manifest): ignored, no panic, rest still counts (Task 1).
4. **Enormous or tiny armor values** (u32 extremes): the 75 % cap holds, level 0 divides safely (Task 1).
5. **Scripted / unattended runs that count exact damage**: `WOWSOULS_GEAR_EFFECT=0` is fully neutral (parse tested in Task 3, behaviour measured in Task 7 step 3); the `die` script's 9999-damage `PlayerHurt` is not written by `npc.rs` and stays untouched (Task 5 note).

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `crates/wow-world/src/gear_effect.rs` | create | pure rules: `armor_reduction`, `weapon_factor`, `block_chip_factor`, `GearEffect` (`from_gear`, `taken`, `reduction`, `summary`) |
| `crates/wow-world/src/lib.rs` | modify | `pub mod gear_effect;` |
| `crates/wow-souls/src/species.rs` | modify | `level: u8` on `SpeciesDef` and the seven species |
| `crates/wow-souls/src/gear_effect.rs` | create | `GearFx` resource, `refresh` system, `WOWSOULS_GEAR_EFFECT` switch, `plugin` |
| `crates/wow-souls/src/main.rs` | modify | `mod gear_effect;` |
| `crates/wow-souls/src/boot.rs` | modify | `.add_plugins(crate::gear_effect::plugin)` |
| `crates/wow-souls/src/hits.rs` | modify | melee damage `* gear.0.melee_factor` |
| `crates/wow-souls/src/projectiles.rs` | modify | projectile damage `* gear.0.ranged_factor` |
| `crates/wow-souls/src/npc.rs` | modify | `GearFx::taken` at both `PlayerHurt` writers |
| `crates/wow-souls/src/equip/ui.rs` | modify | totals line from `GearEffect::summary()` |
| `README.md`, `docs/HANDOVER.md` | modify | the switch row, the entry |

---

### Task 1: The pure rules (`wow-world`)

**Files:**
- Create: `crates/wow-world/src/gear_effect.rs`
- Modify: `crates/wow-world/src/lib.rs` (add `pub mod gear_effect;` after `pub mod equipment;`)
- Test: in-file `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::equipment::{totals, weapon_dps, Gear, Slot, Totals}`, `crate::model::Item` (fields `class`, `delay`, `damage`, `armor`, `block`, `entry`).
- Produces (Tasks 3-6 rely on these exact names):
  - `pub const DISPLAY_LEVEL: u8 = 3;`
  - `pub fn armor_reduction(armor: u32, attacker_level: u8) -> f32`
  - `pub fn weapon_factor(item: &Item) -> f32`
  - `pub fn block_chip_factor(block: u32) -> f32`
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub struct GearEffect { pub armor: u32, pub block: u32, pub melee_factor: f32, pub ranged_factor: f32 }` with `Default` = neutral, `GearEffect::from_gear(gear: &Gear, items: &[Item]) -> GearEffect`, `GearEffect::taken(&self, damage: f32, attacker_level: u8, blocked: bool) -> f32`, `GearEffect::reduction(&self, attacker_level: u8) -> f32`, `GearEffect::summary(&self) -> String`.

- [ ] **Step 1: Declare the module and write the failing tests**

In `crates/wow-world/src/lib.rs` add the line `pub mod gear_effect;` after `pub mod equipment;`.

Create `crates/wow-world/src/gear_effect.rs` with only the tests first (the items above it do not exist yet, so it fails to compile, which is the expected red):

```rust
//! What worn gear does in a fight (spec `docs/superpowers/specs/2026-10-12-gear-effects-design.md`).
//! Pure rules over [`Item`] rows and [`Gear`]: armor lowers the damage taken by WoW 1.12's formula, a
//! weapon's WoW damage per second scales the damage the Dark Souls moveset deals (against the
//! starting sword, damped), a shield's block value shrinks a blocked blow.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equipment::{inv, tests::item, Gear, Slot};

    fn close(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    fn weapon(entry: u32, min: f32, max: f32, delay: u32) -> Item {
        let mut i = item(entry, inv::WEAPON, 2, 7);
        i.damage = (min, max, 0);
        i.delay = delay;
        i
    }

    #[test]
    fn armor_follows_wows_formula_and_caps_at_three_quarters() {
        assert_eq!(armor_reduction(0, 3), 0.0);
        assert!(close(armor_reduction(655, 3), 0.5, 1.0e-6), "655 / (655 + 400 + 255)");
        assert!(close(armor_reduction(275, 3), 275.0 / 930.0, 1.0e-6));
        assert!(close(armor_reduction(100, 0), 0.2, 1.0e-6), "level 0 divides safely");
        assert_eq!(armor_reduction(u32::MAX, 3), ARMOR_CAP);
        assert_eq!(armor_reduction(u32::MAX, 0), ARMOR_CAP);
    }

    #[test]
    fn the_starting_sword_is_exactly_one_and_better_weapons_climb_damped() {
        assert!(close(weapon_factor(&weapon(25, 1.0, 3.0, 1900)), 1.0, 1.0e-5));
        // Militia Dagger 2-4 over 1.5 s = 2.0 dps; Hunting Bow 8-16 over 2.6 s = 4.62 dps.
        assert!(close(weapon_factor(&weapon(2224, 2.0, 4.0, 1500)), 1.3784, 1.0e-3));
        assert!(close(weapon_factor(&weapon(8180, 8.0, 16.0, 2600)), 2.0940, 1.0e-3));
        // Spiked Club 22-34 over 3.1 s = 9.03 dps: capped.
        assert_eq!(weapon_factor(&weapon(4564, 22.0, 34.0, 3100)), WEAPON_FACTOR_MAX);
        // A hopeless weapon is floored, not zeroed.
        assert_eq!(weapon_factor(&weapon(9, 0.1, 0.1, 3000)), WEAPON_FACTOR_MIN);
    }

    #[test]
    fn a_row_with_no_swing_time_or_no_damage_changes_nothing() {
        assert_eq!(weapon_factor(&weapon(1, 5.0, 9.0, 0)), 1.0);
        assert_eq!(weapon_factor(&weapon(2, 0.0, 0.0, 2000)), 1.0);
        let vest = item(3, inv::CHEST, 4, 2);
        assert_eq!(weapon_factor(&vest), 1.0);
    }

    #[test]
    fn block_takes_five_percent_a_point_off_the_chip_up_to_half() {
        assert_eq!(block_chip_factor(0), 1.0);
        assert!(close(block_chip_factor(1), 0.95, 1.0e-6));
        assert!(close(block_chip_factor(5), 0.75, 1.0e-6));
        assert_eq!(block_chip_factor(100), 0.5);
    }

    #[test]
    fn taken_applies_armor_always_and_block_only_to_a_blocked_blow() {
        let none = GearEffect::default();
        assert_eq!(none.taken(25.0, 4, false), 25.0);
        assert_eq!(none.taken(25.0, 4, true), 25.0);

        let worn = GearEffect {
            armor: 195,
            block: 5,
            melee_factor: 1.0,
            ranged_factor: 1.0,
        };
        // 195 / (195 + 400 + 340) = 0.2086 off a level 4 blow.
        assert!(close(worn.taken(25.0, 4, false), 25.0 * (1.0 - 195.0 / 935.0), 1.0e-3));
        // A blocked chip of 12.5 also loses a quarter to the shield.
        let armored = worn.taken(12.5, 4, false);
        assert!(close(worn.taken(12.5, 4, true), armored * 0.75, 1.0e-4));
    }

    #[test]
    fn the_start_kit_is_armor_seven_and_no_change_to_damage() {
        let sword = weapon(25, 1.0, 3.0, 1900);
        let mut shield = item(2362, inv::SHIELD, 4, 6);
        shield.armor = 5;
        shield.block = 1;
        let mut pants = item(39, inv::LEGS, 4, 1);
        pants.armor = 2;
        let items = vec![sword, shield, pants];
        let mut gear = Gear::default();
        gear.set(Slot::MainHand, Some(25));
        gear.set(Slot::OffHand, Some(2362));
        gear.set(Slot::Legs, Some(39));
        let fx = GearEffect::from_gear(&gear, &items);
        assert_eq!((fx.armor, fx.block), (7, 1));
        assert!(close(fx.melee_factor, 1.0, 1.0e-5));
        assert_eq!(fx.ranged_factor, 1.0);
        assert_eq!(fx.summary(), "Armor 7 (-1 %)     Block 1");
    }

    #[test]
    fn a_ranged_weapon_scales_only_the_ranged_factor() {
        let items = vec![weapon(25, 1.0, 3.0, 1900), {
            let mut bow = weapon(8180, 8.0, 16.0, 2600);
            bow.inventory_type = inv::RANGED;
            bow.subclass = 2;
            bow
        }];
        let mut gear = Gear::default();
        gear.set(Slot::MainHand, Some(25));
        gear.set(Slot::Ranged, Some(8180));
        let fx = GearEffect::from_gear(&gear, &items);
        assert!(close(fx.melee_factor, 1.0, 1.0e-5));
        assert!(close(fx.ranged_factor, 2.0940, 1.0e-3));
    }

    #[test]
    fn empty_gear_and_unknown_items_are_neutral() {
        assert_eq!(GearEffect::from_gear(&Gear::default(), &[]), GearEffect::default());
        let mut gear = Gear::default();
        gear.set(Slot::MainHand, Some(999_999));
        gear.set(Slot::Chest, Some(888_888));
        assert_eq!(GearEffect::from_gear(&gear, &[]), GearEffect::default());
        assert_eq!(GearEffect::default().summary(), "Armor 0");
    }

    #[test]
    fn the_summary_quotes_the_level_three_figure_and_the_weapon() {
        let fx = GearEffect {
            armor: 275,
            block: 3,
            melee_factor: 1.378,
            ranged_factor: 1.0,
        };
        assert_eq!(fx.summary(), "Armor 275 (-30 %)     Block 3     Hits x1.38");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world gear_effect`
Expected: compile errors (`cannot find function armor_reduction`, `GearEffect` ...).

- [ ] **Step 3: Write the implementation**

Insert above the `#[cfg(test)]` line of `crates/wow-world/src/gear_effect.rs`:

```rust
use crate::equipment::{totals, weapon_dps, Gear, Slot};
use crate::model::Item;

/// WoW 1.12's armor formula: `armor / (armor + 400 + 85 x attacker level)`.
pub const ARMOR_BASE: f32 = 400.0;
pub const ARMOR_PER_LEVEL: f32 = 85.0;
/// The most damage armor can take off.
pub const ARMOR_CAP: f32 = 0.75;
/// The Worn Shortsword's damage per second ((1 + 3) / 2 over 1.9 s): the starting kit is exactly x1.
pub const REF_DPS: f32 = 2.0 / 1.9;
/// How damped the step between weapons is: factor = (dps / REF_DPS) ^ this.
pub const WEAPON_EXPONENT: f32 = 0.5;
pub const WEAPON_FACTOR_MIN: f32 = 0.75;
pub const WEAPON_FACTOR_MAX: f32 = 2.5;
/// Each point of a shield's block value takes this share off a blocked blow's chip damage ...
pub const BLOCK_REDUCTION_PER_POINT: f32 = 0.05;
/// ... up to this share.
pub const BLOCK_REDUCTION_MAX: f32 = 0.5;
/// The attacker level the equipment page quotes its armor figure at: the valley's typical creature.
pub const DISPLAY_LEVEL: u8 = 3;

/// The share of a blow that armor takes off, from a creature of `attacker_level`.
pub fn armor_reduction(armor: u32, attacker_level: u8) -> f32 {
    let armor = armor as f32;
    let against = armor + ARMOR_BASE + ARMOR_PER_LEVEL * f32::from(attacker_level);
    (armor / against).min(ARMOR_CAP)
}

/// How much harder (or softer) this weapon's blows are than the starting sword's: its WoW damage per
/// second against [`REF_DPS`], damped. A row without a swing time or damage, or that is not a weapon,
/// is 1.
pub fn weapon_factor(item: &Item) -> f32 {
    if item.class != 2 {
        return 1.0;
    }
    let dps = weapon_dps(item);
    if dps <= 0.0 {
        return 1.0;
    }
    (dps / REF_DPS)
        .powf(WEAPON_EXPONENT)
        .clamp(WEAPON_FACTOR_MIN, WEAPON_FACTOR_MAX)
}

/// What is left of a blocked blow's chip after the shield's block value.
pub fn block_chip_factor(block: u32) -> f32 {
    1.0 - (BLOCK_REDUCTION_PER_POINT * block as f32).min(BLOCK_REDUCTION_MAX)
}

/// What the worn gear does in a fight, summed once whenever the gear changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GearEffect {
    /// Every worn piece's armor (the shield's too).
    pub armor: u32,
    /// The shield's block value.
    pub block: u32,
    /// The main hand's [`weapon_factor`] (1 with empty hands).
    pub melee_factor: f32,
    /// The ranged slot's [`weapon_factor`] (1 with nothing there).
    pub ranged_factor: f32,
}

impl Default for GearEffect {
    /// Nothing worn: no change to any blow.
    fn default() -> Self {
        Self {
            armor: 0,
            block: 0,
            melee_factor: 1.0,
            ranged_factor: 1.0,
        }
    }
}

impl GearEffect {
    pub fn from_gear(gear: &Gear, items: &[Item]) -> Self {
        let t = totals(gear, items);
        let factor = |slot: Slot| {
            gear.get(slot)
                .and_then(|entry| items.iter().find(|i| i.entry == entry))
                .map_or(1.0, weapon_factor)
        };
        Self {
            armor: t.armor,
            block: t.block,
            melee_factor: factor(Slot::MainHand),
            ranged_factor: factor(Slot::Ranged),
        }
    }

    /// The share armor takes off a blow of `attacker_level`.
    pub fn reduction(&self, attacker_level: u8) -> f32 {
        armor_reduction(self.armor, attacker_level)
    }

    /// The damage that gets through: armor always, the shield's block only when the blow was blocked
    /// (`damage` is then already the guard's chip).
    pub fn taken(&self, damage: f32, attacker_level: u8, blocked: bool) -> f32 {
        let mut taken = damage * (1.0 - self.reduction(attacker_level));
        if blocked {
            taken *= block_chip_factor(self.block);
        }
        taken
    }

    /// The equipment page's totals line.
    pub fn summary(&self) -> String {
        let mut line = format!("Armor {}", self.armor);
        let off = (self.reduction(DISPLAY_LEVEL) * 100.0).round();
        if off >= 1.0 {
            line.push_str(&format!(" (-{off:.0} %)"));
        }
        if self.block > 0 {
            line.push_str(&format!("     Block {}", self.block));
        }
        if (self.melee_factor - 1.0).abs() >= 0.005 {
            line.push_str(&format!("     Hits x{:.2}", self.melee_factor));
        }
        line
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world gear_effect`
Expected: 9 tests pass. If `the_summary_quotes...` fails on the rounding (275 / 930 = 29.57 % -> `-30 %`), the test is right: `round()` gives 30.

- [ ] **Step 5: Gates for the crate**

Run: `cd /c/Spel/wow-souls && cargo fmt -p wow-world -- --check && cargo clippy -j 4 -p wow-world --all-targets -- -D warnings`
Expected: both exit 0 (fix formatting with `cargo fmt -p wow-world`). No commit.

---

### Task 2: A level for every species

**Files:**
- Modify: `crates/wow-souls/src/species.rs` (struct at line 29-75, the seven consts at lines 98-280)
- Test: the existing `#[cfg(test)] mod tests` at the end of `species.rs` (line 302)

**Interfaces:**
- Consumes: `by_name`, `ROSTER` in `species.rs`.
- Produces: `SpeciesDef.level: u8` (Task 5 reads `species.0.level`).

- [ ] **Step 1: Write the failing test**

Append inside the existing `mod tests` of `crates/wow-souls/src/species.rs`:

```rust
    /// `creature_template.level_max` of each entry in `cache/world/northshire.txt`:
    /// Kobold Laborer (80) 4, Vermin (6) 2, Worker (257) 3, Timber Wolf (69) 2, Young Wolf (299) 1,
    /// Defias Thug (38) 4, Garrick Padfoot (103) 5.
    #[test]
    fn every_species_carries_its_database_level() {
        let levels = [
            ("kobold", 4),
            ("vermin", 2),
            ("worker", 3),
            ("wolf", 2),
            ("young_wolf", 1),
            ("thug", 4),
            ("garrick", 5),
        ];
        for (name, level) in levels {
            assert_eq!(by_name(name).expect(name).level, level, "{name}");
        }
        assert_eq!(levels.len(), ROSTER.len(), "a species without a level row");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-souls every_species_carries_its_database_level`
Expected: compile error `no field level on type SpeciesDef`.

- [ ] **Step 3: Add the field and the seven values**

In `SpeciesDef`, after the `pub title: &'static str,` line add:

```rust
    /// The creature's WoW level (`creature_template.level_max` of its entry): the attacker level of the
    /// armor formula ([`wow_world::gear_effect::armor_reduction`]).
    pub level: u8,
```

In each const, add a `level: N,` line directly after its `title:` line (the `title` lines are unique):

| `title:` line to match | line to insert after it |
|---|---|
| `    title: "Kobold",` | `    level: 4,` |
| `    title: "Timber Wolf",` | `    level: 2,` |
| `    title: "Defias Thug",` | `    level: 4,` |
| `    title: "Kobold Vermin",` | `    level: 2,` |
| `    title: "Kobold Worker",` | `    level: 3,` |
| `    title: "Young Wolf",` | `    level: 1,` |
| `    title: "Garrick Padfoot",` | `    level: 5,` |

- [ ] **Step 4: Run to verify it passes**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-souls every_species_carries_its_database_level`
Expected: PASS (compiles: any other `SpeciesDef { .. }` literal would have failed; the grep found only these seven).

- [ ] **Step 5: Gates**

Run: `cd /c/Spel/wow-souls && cargo fmt -p wow-souls -- --check && cargo clippy -j 4 -p wow-souls --all-targets -- -D warnings`
Expected: exit 0. No commit.

---

### Task 3: The game resource and its switch

**Files:**
- Create: `crates/wow-souls/src/gear_effect.rs`
- Modify: `crates/wow-souls/src/main.rs` (add `mod gear_effect;` between `mod friendly;` and `mod gfx;`), `crates/wow-souls/src/boot.rs` (add `.add_plugins(crate::gear_effect::plugin)` after the `crate::equip::plugin` line, ~line 97)
- Test: in-file

**Interfaces:**
- Consumes: `wow_world::gear_effect::GearEffect`, `crate::loot::{Data, Owned}` (`owned.progress.gear: Gear`, `data.0.items: Vec<Item>`).
- Produces: `pub struct GearFx(pub GearEffect)` (a `Resource`, `Default` neutral); `pub fn plugin(app: &mut App)`; env switch `WOWSOULS_GEAR_EFFECT=0`.

- [ ] **Step 1: Write the failing test**

Create `crates/wow-souls/src/gear_effect.rs` with only:

```rust
//! Worn gear in a fight: the [`GearFx`] resource the damage hooks read (`hits.rs`, `projectiles.rs`,
//! `npc.rs`), kept current from the items save and the item table. The rules are
//! `wow_world::gear_effect`; the spec is `docs/superpowers/specs/2026-10-12-gear-effects-design.md`.
//!
//! `WOWSOULS_GEAR_EFFECT=0` makes gear do nothing in a fight (scripted checks that count exact damage).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_explicit_zero_turns_gear_off() {
        assert!(switch_on(None));
        assert!(switch_on(Some("1")));
        assert!(switch_on(Some("")));
        assert!(!switch_on(Some("0")));
        assert!(!switch_on(Some(" 0 ")));
    }

    #[test]
    fn the_resource_starts_neutral() {
        let fx = GearFx::default().0;
        assert_eq!((fx.armor, fx.block), (0, 0));
        assert_eq!((fx.melee_factor, fx.ranged_factor), (1.0, 1.0));
    }
}
```

Add `mod gear_effect;` to `main.rs`.

- [ ] **Step 2: Run to verify it fails**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-souls gear_effect`
Expected: compile errors (`switch_on`, `GearFx` not found).

- [ ] **Step 3: Write the implementation**

Insert above `#[cfg(test)]` in `crates/wow-souls/src/gear_effect.rs`:

```rust
use bevy::prelude::*;
use wow_world::gear_effect::GearEffect;

use crate::loot::{Data, Owned};

/// What the avatar's gear does now. Neutral until the save and the item table are loaded.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct GearFx(pub GearEffect);

/// Whether gear counts (`WOWSOULS_GEAR_EFFECT`, default yes).
#[derive(Resource, Debug, Clone, Copy)]
struct Enabled(bool);

/// `WOWSOULS_GEAR_EFFECT=0` is the only value that turns gear off.
fn switch_on(value: Option<&str>) -> bool {
    !value.is_some_and(|v| v.trim() == "0")
}

/// Recomputes the effect when the worn gear, the item table or the switch changed, and says so.
fn refresh(
    owned: Option<Res<Owned>>,
    data: Option<Res<Data>>,
    enabled: Res<Enabled>,
    mut fx: ResMut<GearFx>,
) {
    let (Some(owned), Some(data)) = (owned, data) else {
        return;
    };
    if !owned.is_changed() && !data.is_changed() && !enabled.is_changed() {
        return;
    }
    let now = if enabled.0 {
        GearEffect::from_gear(&owned.progress.gear, &data.0.items)
    } else {
        GearEffect::default()
    };
    if fx.0 != now {
        fx.0 = now;
        info!(
            "gear: armor {} ({:.0} % off a level {} blow), melee x{:.2}, ranged x{:.2}, block {}{}",
            now.armor,
            now.reduction(wow_world::gear_effect::DISPLAY_LEVEL) * 100.0,
            wow_world::gear_effect::DISPLAY_LEVEL,
            now.melee_factor,
            now.ranged_factor,
            now.block,
            if enabled.0 { "" } else { " (switched off)" }
        );
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<GearFx>()
        .insert_resource(Enabled(switch_on(
            std::env::var("WOWSOULS_GEAR_EFFECT").ok().as_deref(),
        )))
        .add_systems(Update, refresh);
}
```

In `boot.rs`, after `.add_plugins(crate::equip::plugin)` add `.add_plugins(crate::gear_effect::plugin)`.

Note: when the switch is off from the start the first computed value equals `default()`, so `fx.0 != now` is false and nothing is logged; that is fine (the resource is already neutral).

- [ ] **Step 4: Run to verify it passes**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-souls gear_effect`
Expected: 2 tests pass; `cargo build -j 4 -p wow-souls` compiles.

- [ ] **Step 5: Gates**

Run: `cd /c/Spel/wow-souls && cargo fmt -p wow-souls -- --check && cargo clippy -j 4 -p wow-souls --all-targets -- -D warnings`
Expected: exit 0. No commit.

---

### Task 4: Damage dealt (melee and ranged)

**Files:**
- Modify: `crates/wow-souls/src/hits.rs` (`strike`, params at line ~359, damage at lines ~444-449)
- Modify: `crates/wow-souls/src/projectiles.rs` (`release`, params at line 39-47, `damage:` at line 76)

**Interfaces:**
- Consumes: `crate::gear_effect::GearFx` (`.0.melee_factor`, `.0.ranged_factor`).
- Produces: nothing new.

Both files are co-owned: re-read each right before editing.

- [ ] **Step 1: Melee hook in `hits.rs`**

Re-read `hits.rs` lines 336-460. Add a parameter after `buffs`:

```rust
    buffs: Res<crate::abilities::Buffs>,
    gear: Res<crate::gear_effect::GearFx>,
```

and change the damage expression (comment and last line only):

```rust
        // Battle Shout (`crate::abilities`) and the weapon's own worth (`crate::gear_effect`) scale it.
        let damage = if swing.ds {
            moves.as_ref().map_or(LIGHT_DAMAGE, |m| m.base_damage) * swing.spec.mult
        } else {
            hit_damage(&swing.spec)
        } * buffs.damage_factor()
            * gear.0.melee_factor;
```

(The existing `hit:` log line prints the final `damage`, so the factor is visible in every hit line.)

- [ ] **Step 2: Ranged hook in `projectiles.rs`**

Re-read `projectiles.rs` lines 38-85. Add a parameter to `release`:

```rust
    gear: Res<crate::gear_effect::GearFx>,
```

(after `lock: Res<LockOn>,`) and change the spawn field:

```rust
        damage: spec.damage * gear.0.ranged_factor,
```

- [ ] **Step 3: Build and run the existing tests**

Run: `cd /c/Spel/wow-souls && cargo build -j 4 -p wow-souls && cargo test -j 4 -p wow-souls hits projectiles`
Expected: builds; the existing hit and projectile tests pass unchanged (no test harness builds `strike` or `release` as a system; if one panics for a missing `GearFx`, add `app.init_resource::<crate::gear_effect::GearFx>()` to that harness). No new unit test: each hook is one multiply by a value tested in Task 1, and Task 7 measures it live.

- [ ] **Step 4: Gates**

Run: `cd /c/Spel/wow-souls && cargo fmt -p wow-souls -- --check && cargo clippy -j 4 -p wow-souls --all-targets -- -D warnings`
Expected: exit 0. No commit.

---

### Task 5: Damage taken (armor and block, at the source)

**Files:**
- Modify: `crates/wow-souls/src/npc.rs` — the `think` system (params at lines ~836-857, the old strike's `PlayerHurt` at lines ~1124-1132) and the `strike` system (params at lines ~1149-1177, the write at lines ~1267-1279)

**Interfaces:**
- Consumes: `crate::gear_effect::GearFx` (`.0.taken(damage, level, blocked)`), `Species.0.level` (Task 2).
- Produces: nothing new.

Where `PlayerHurt.damage` is made is where armor goes, so `shake`, `blood`, `audio`, `vocal` (which read `hurt.damage`) all see the reduced number. `bloodstain.rs`'s `die` script writes its own 9999 `PlayerHurt` and is deliberately left alone.

- [ ] **Step 1: The kit's blow (`strike`)**

Add a parameter to `strike` after `defense`:

```rust
    gear: Res<crate::gear_effect::GearFx>,
```

Replace the block that builds and writes the hurt (from `let damage = (a.spec.damage * species.0.damage_scale).round();` to the end of the `hurts.write(...)`) so that the guard runs first and gear second, and the log prints the final figure:

```rust
        let damage = (a.spec.damage * species.0.damage_scale).round();
        let (damage, blocked) = crate::defense::intercept(
            defense.as_deref(),
            defense
                .as_deref()
                .and_then(|k| k.attack(species.0.kit, &a.spec.name)),
            player,
            at.translation(),
            &mut action,
            &mut stamina,
            damage,
        );
        // Armor always, a shield's block value only when the guard caught it.
        let damage = gear.0.taken(damage, species.0.level, blocked);
        hurts.write(PlayerHurt {
            from: e,
            damage,
            level: a.spec.level,
            stop: if blocked {
                crate::hurt::HURT_STOP
            } else {
                a.spec.stop
            },
            knock: a.spec.knock,
            poise: a.spec.poise,
            blocked,
        });
```

(The `info!` below it already prints `{damage:.0}`; leave it.)

- [ ] **Step 2: The old blow (`think`, no Dark Souls kit)**

Add a parameter to `think` after `rng`:

```rust
    gear: Res<crate::gear_effect::GearFx>,
```

and in the old strike's write change `damage: spec.damage,` to:

```rust
            damage: gear.0.taken(spec.damage, species.0.level, false),
```

(`species` is the loop variable of the same `for` in `think`; `spec` is the old swing spec already in scope there.)

- [ ] **Step 3: Build and run the npc tests**

Run: `cd /c/Spel/wow-souls && cargo build -j 4 -p wow-souls && cargo test -j 4 -p wow-souls npc`
Expected: builds and passes. If `think` or `strike` is exercised by a test harness that builds an `App`, add `init_resource::<crate::gear_effect::GearFx>()` there.

- [ ] **Step 4: Gates**

Run: `cd /c/Spel/wow-souls && cargo fmt -p wow-souls -- --check && cargo clippy -j 4 -p wow-souls --all-targets -- -D warnings`
Expected: exit 0. No commit.

---

### Task 6: The equipment page says what the gear does

**Files:**
- Modify: `crates/wow-souls/src/equip/ui.rs` (the `Words::Totals` arm, lines ~789-796, and `let totals = equipment::totals(gear, items);` at line ~773)

**Interfaces:**
- Consumes: `wow_world::gear_effect::GearEffect::{from_gear, summary}` (Task 1).

- [ ] **Step 1: Replace the totals line**

Re-read `equip/ui.rs` lines 765-800. Replace the `Words::Totals` arm with:

```rust
            Words::Totals => (
                wow_world::gear_effect::GearEffect::from_gear(gear, items).summary(),
                IVORY,
            ),
```

and delete the line `let totals = equipment::totals(gear, items);` if nothing else in the function uses `totals` (clippy `-D warnings` flags an unused variable; if `equipment` is then an unused import in the file, remove it from the `use`).

Note for the user's debug runs: the page quotes what the rule would give; with `WOWSOULS_GEAR_EFFECT=0` the fight ignores it.

- [ ] **Step 2: Build, test, gates**

Run: `cd /c/Spel/wow-souls && cargo build -j 4 -p wow-souls && cargo test -j 4 -p wow-souls equip && cargo fmt -p wow-souls -- --check && cargo clippy -j 4 -p wow-souls --all-targets -- -D warnings`
Expected: all exit 0 (the `summary` format itself is pinned by Task 1's tests). No commit.

---

### Task 7: Measure it live, then document

**Files:**
- Modify: `README.md` (one switch row after the `WOWSOULS_KIT=0` row, line ~97), `docs/HANDOVER.md` (the entry under "Next action": replace the "waiting for the user's review; no code" wording with the built state and the measured numbers)
- Evidence: log lines pasted into the HANDOVER entry; screenshots only if cheap (`docs/evidence/gear-*.png`)

**Interfaces:** none.

Unattended runs use scratch saves so the user's own saves are never touched. All commands from `/c/Spel/wow-souls` after `cargo build -j 4 -p wow-souls`.

- [ ] **Step 1: Scratch environment**

```bash
cd /c/Spel/wow-souls
export S=$(mktemp -d)
export WOWSOULS_SETTINGS=$S/settings.txt WOWSOULS_CHARACTER=$S/character.txt WOWSOULS_ITEMS_SAVE=$S/items.txt WOWSOULS_QUESTS_SAVE=$S/quests.txt WOWSOULS_SOULS_SAVE=$S/souls.txt WOWSOULS_MENU=0
```

- [ ] **Step 2: Armor, start kit then a leather set**

Run (a still player is struck by one kobold at 3 yd; the kobold's blow is 25 before gear):

```bash
WOWSOULS_AT=-8906,-136 WOWSOULS_KOBOLDS=1 WOWSOULS_KOBOLD_AT=-8909,-136 WOWSOULS_SPIN=0 WOWSOULS_TRACE=1 timeout 60 ./target/debug/wow-souls.exe 2>&1 | grep -E "gear:|strikes the player"
```

Expected: `gear: armor 7 (1 % off a level 3 blow), melee x1.00, ranged x1.00, block 1` and kobold strikes for 25 (24.8 rounds to 25: the kobold's level is 4, 7 / 747 = 0.9 %).

Then with a leather set and the Large Round Shield (armor 33 + 29 + 23 + 19 + 21 + 15 + 55 = 195):

```bash
WOWSOULS_EQUIP=2127,2126,2123,2122,2125,2124,2129 WOWSOULS_AT=-8906,-136 WOWSOULS_KOBOLDS=1 WOWSOULS_KOBOLD_AT=-8909,-136 WOWSOULS_SPIN=0 WOWSOULS_TRACE=1 timeout 60 ./target/debug/wow-souls.exe 2>&1 | grep -E "gear:|strikes the player"
```

Expected: a `gear: armor 195 ...` line and the kobold's strike prints about 20 (25 x (1 - 195 / 935) = 19.8). Record both outputs.

- [ ] **Step 3: The switch is neutral**

Same command as the leather run with `WOWSOULS_GEAR_EFFECT=0` in front. Expected: no `gear:` line with armor 195 (the resource stays neutral) and the kobold strikes for 25.

- [ ] **Step 4: Weapon scaling, same run with and without**

```bash
WOWSOULS_EQUIP=2224 WOWSOULS_AT=-8906,-136 WOWSOULS_KOBOLD_AT=-8909,-136 WOWSOULS_KOBOLDS=1 WOWSOULS_ATTACK=4.0,0.9 WOWSOULS_SPIN=0 WOWSOULS_TRACE=1 timeout 60 ./target/debug/wow-souls.exe 2>&1 | grep -E "gear:|hit:"
WOWSOULS_GEAR_EFFECT=0 WOWSOULS_EQUIP=2224 WOWSOULS_AT=-8906,-136 WOWSOULS_KOBOLD_AT=-8909,-136 WOWSOULS_KOBOLDS=1 WOWSOULS_ATTACK=4.0,0.9 WOWSOULS_SPIN=0 WOWSOULS_TRACE=1 timeout 60 ./target/debug/wow-souls.exe 2>&1 | grep -E "gear:|hit:"
```

Expected: the first prints `melee x1.38` and its `hit:` damage is 1.38 times the second's (same dagger family, same move). If no hit lands in 4-6 s, lengthen `WOWSOULS_ATTACK` start or move the kobold closer.

- [ ] **Step 5: Block**

Re-run Step 2's leather command with `WOWSOULS_LOCK=...` is not needed; use the existing guard script switch from the handover's defense work if one exists (`grep -rn "WOWSOULS_BLOCK\|WOWSOULS_GUARD" crates/wow-souls/src`). Expected: a blocked blow's printed chip is the guard's 12.5 x (1 - 195/935) x 0.75 for block 5. If no scripted guard switch exists, state that in the HANDOVER entry as "block verified by unit tests only; user to try E at a kobold".

- [ ] **Step 6: Whole-workspace gates**

```bash
cd /c/Spel/wow-souls
cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check
cargo clippy -j 4 --workspace --all-targets -- -D warnings
DSR_DIR="C:/Spel/Dark Souls Remastered" WOW_DATA="C:/Spel/World of Warcraft 1.12/Data" cargo test -j 4 --workspace
```

Expected: all exit 0; test counts up by the 9 + 1 + 2 new tests (wow-world 159 -> 168, wow-souls 687 -> 690).

- [ ] **Step 7: Documentation**

Add to `README.md` after the `WOWSOULS_KIT=0` row:

```
| `WOWSOULS_GEAR_EFFECT=0` | worn gear does nothing in a fight (default on: armor lowers damage taken by WoW's formula, a weapon's dps scales the Dark Souls moveset's damage, a shield's block value shrinks a blocked blow; see `gear_effect.rs`) |
```

Update the HANDOVER entry ("Gear must do something ...") to: BUILT, with the measured Step 2-5 numbers, the three spec deviations (listed at the top of this plan), and "next: stats (+Str / +Stam; importer columns `stat_type1..10` / `stat_value1..10`; the user said stats are the step after this one)". Nothing committed.

---

## Self-review (done when writing)

- **Spec coverage:** armor rule (Tasks 1, 5), weapon factor melee + ranged (Tasks 1, 4), block (Tasks 1, 5), `SpeciesDef.level` (Task 2), recomputed on gear change + log line (Task 3), switch (Task 3), display (Task 6), tests / live / gates (Tasks 1-7), save format unchanged (nothing touches saves), stats and armor-to-poise out of scope (noted in Task 7's handover text). Spec's `DEFAULT_ATTACKER_LEVEL` is dropped: every creature has a species, so every creature has a level.
- **Placeholders:** none; every code step has the code. Task 7 Step 5 names its fallback honestly.
- **Type consistency:** `GearEffect { armor, block, melee_factor, ranged_factor }`, `GearEffect::{from_gear, taken, reduction, summary}`, `GearFx(pub GearEffect)`, `DISPLAY_LEVEL`, `SpeciesDef.level: u8` are used with the same names and signatures in Tasks 1-7.
