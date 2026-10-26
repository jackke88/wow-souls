# Mage class Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A human male Mage can be made in the creation menu and played: a mana bar, four spells (Fireball and Frost Armor known at the start, Frostbolt and Fire Blast from Khelden Bremen), cast with Dark Souls' own cast animations, WoW's spell models and sounds, and a missile that hits, slows, burns and pays out XP, souls and loot like a sword blow.

**Architecture:** A class is a data row (`wow_world::class`), Warrior row 1 unchanged, Mage row 2. Mana is a second resource beside `Rage`. A cast is a new `ActionState::Cast` in the pure action machine, posed by DS cast clips that a new extractor family (`casts`) retargets and publishes as `cache/dsr/casts.txt`; at the clip's release window a missile (the arrow's flight generalized with homing) or an instant hit leaves the hand and writes the same `Hit` a sword blow does.

**Tech Stack:** Rust 1.98.1, Bevy 0.18.1 on benilla's engine, the workspace crates `wow-world` (pure rules), `wow-souls` (game), `dsr-cache` / `dsr-formats` (extractor), `wscl` (clip files).

**Spec:** `docs/superpowers/specs/2026-10-14-mage-class-design.md` (read it first; this plan quotes its numbers).

## Decisions made while planning (the spec is updated in Task 4, step 12)

1. **Missile flight is WoW's, steering is Dark Souls'.** WoW gives speed (24 / 28 yd/s, `spell_template.speed`) and range (35 / 30 yd, `SpellRange`); the DS `Bullet` row gives the homing angle, the hit radius and the impact burst. No gravity: DS's Fireball is a 10-m lob (15 m/s, gravity 9.8) that cannot reach WoW's 35 yd.
2. **First clip picks** come from a TAE survey of `a00_6200-6226` (clips with the flag 91 / 90 / 89 windows): Fireball `a00_6205`, Frostbolt `a00_6209`, Fire Blast `a00_6207` (0.97 s, release at 0.13 s), Frost Armor `a00_6217`. `a00_6200` (the Soul Arrow type) and `a00_6206` (Iron Flesh) have no flag-90 window and are not usable by the release rule. Task 3's survey confirms or swaps them.
3. **`casts.txt` is its own publication** (like `interaction.txt` / `locomotion.txt`), written by `wow-souls-dsr casts` and as the last step of `extract`.
4. **An instant spell needs its target before it is paid for**: no target / out of range refuses with nothing spent.
5. **Kits, trainer and spell numbers are constants** with the query in a comment and a test that pins them (the warrior's way): no exporter work.
6. **S7 (drinking water, balance) gets its own plan** once the user has played S1-S6 and judged the cast in motion.

## Global Constraints

- **Commit only when the user asks.** Every "Gate" step below replaces a commit. Never `git commit` in this plan.
- **Never `cargo fmt --all`** (it rewrites the benilla fork). Format gate: `cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check`.
- **Build and test with `-j 4`** (RAM-tight). Clippy gate: `cargo clippy -j 4 --workspace --all-targets -- -D warnings`.
- **Install tests** silently skip unless `DSR_DIR` and `WOW_DATA` are set in the same shell command: `DSR_DIR="C:/Spel/Dark Souls Remastered" WOW_DATA="C:/Spel/World of Warcraft 1.12/Data" cargo test -j 4 --workspace`. Work in `C:\Spel\wow-souls`, bash syntax (`cd /c/Spel/wow-souls`).
- **Warrior behaviour never changes**: every existing warrior test stays green; last green counts (2026-10-13): wow-souls 807, wow-world 203, dsr-cache 68, dsr-formats 52, wscl 4.
- **Enemy damage is not scaled** for the new class ("Souls ska vara farligt").
- **Co-owned files** (another chat edits them): `hits.rs`, `projectiles.rs`, `defense.rs`, `hurt.rs`, `gear_effect.rs`, `npc.rs`, `combat/*`. **Re-read a file right before editing it; keep edits small.**
- **No new hardcoded `C:\Spel\...` path** in code: use `crate::dsclip::cache_dir()` and the existing env switches. The DSR install and the WoW client are read-only.
- **WoW's own visuals and sounds**; an effect model is never an `ActorModel`: spawn through `abilities::fx::spawn` (`loot::vfx::effect`).
- **Python hangs the Bash tool**: use perl / awk / sed or the Edit / Write tools for scripted edits.
- **Judge animation in motion**: a dense frame series from the game's camera (and a front view), never stills.
- **Unattended runs must point every save at scratch copies** (`WOWSOULS_{CLOCK,ITEMS,QUESTS,SOULS}_SAVE`, `WOWSOULS_CHARACTER`) or they write the user's real saves; they are silent unless `WOWSOULS_AUDIO=1|null`.
- After each task, update `docs/HANDOVER.md` "Next / open" item 1 in place (a line or two; the file stays under 8 KB) and put the story in `docs/log/2026-10-14-mage-<task>.md`.

## Review Focus

Failure modes the spec implies that no happy-path test would catch; each has a test in the task that owns the code:

1. **A cast with no Dark Souls cache** (no `casts.txt`, or the common manifest is stale): the spell refuses ("You cannot do that yet."), nothing is spent, no panic. (Task 4, `plan` test.)
2. **A blow in the wind-up**: the cast ends, the mana stays spent, no missile appears. (Task 4, `release` guard test and the action-machine test.)
3. **The target dies or despawns mid-flight**: the missile flies on straight and ends at its range; it never panics on a stale `Entity` (the steering reads the target through a fallible `get`). (Task 4: `steer` tests, and the live check in Step 11 kills the target mid-flight.)
4. **A warrior at the mage trainer, a mage at the warrior's**: no lesson list, and a forged `Request::Train` for another class's spell is refused. (Task 6.)
5. **Max mana shrinks below the current pool** (the gear that gave Intellect comes off): the pool clamps, never negative, never above max; a level-up lengthens the bar without a free refill. (Task 2, `Mana::sync` tests.)

---

## File structure

| File | Responsibility |
|---|---|
| `crates/wow-world/src/class.rs` (new) | `Power`, `ClassDef`, the Warrior and Mage rows, `def` / `trainer_of` / `of_spell` |
| `crates/wow-world/src/spells.rs` (new, Task 4) | The four spells' WoW numbers: delivery, damage range, dot, slow, armor, range, cooldown, `roll` |
| `crates/wow-world/src/abilities.rs` | The abilities (7), `Power` + `cost`, class-aware `known` / `trainer_list` |
| `crates/wow-world/src/equipment.rs` | `CLASS_MAGE`, `HUMAN_MAGE_KIT`, `start_kit` over the class rows |
| `crates/wow-world/src/attributes.rs` | `intellect_mana`, `Character.max_mana` |
| `crates/wow-world/src/quest/mod.rs` | `Class::from_id`, `Player::of_class` |
| `crates/wow-world/src/gear_effect.rs` | `GearEffect.buff_armor` (Frost Armor) |
| `crates/wow-souls/src/character.rs` | the Mage becomes creatable |
| `crates/wow-souls/src/quests/mod.rs` | the quest game's player carries the chosen class; `services` gets the class |
| `crates/wow-souls/src/abilities/mod.rs` | `Mana`, `ActiveClass`, `activate` over two powers, the spell pre-checks and cast start |
| `crates/wow-souls/src/abilities/ui.rs` | the mana bar, slot dimming by power |
| `crates/wow-souls/src/abilities/fx.rs` | the mage's WoW models and sound kits |
| `crates/wow-souls/src/abilities/moves.rs` | `Slowed.factor`, the cooldowns of the spells |
| `crates/wow-souls/src/level.rs` | the mana pool follows level and gear |
| `crates/wow-souls/src/combat/action.rs` | `ActionState::Cast`, `Cast`, `CastSpec`, `Action::can_cast` / `begin_cast` |
| `crates/wow-souls/src/combat/mod.rs` | the cast roots the body; the debug trace |
| `crates/wow-souls/src/dsclip.rs` | `ClipId::Cast`, the cast kit in `clips_of` and `pose` |
| `crates/wow-souls/src/dsview.rs` | `WOWSOULS_DSVIEW=<role>` shows a cast clip |
| `crates/wow-souls/src/projectiles.rs` | the arrow's flight generalized: homing, spell flight, `SpellHit` |
| `crates/wow-souls/src/spells/mod.rs` (new) | `plan`, `aim_instant`, `release`, `spell_hit`, `burn`, `frost_armor`, the plugin |
| `crates/wow-souls/src/spells/cast.rs` (new) | `Role`, `Timing`, `Bullet`, `Kit` (loads `casts.txt`) |
| `crates/wow-souls/src/shop/mod.rs` | trainer class gate |
| `crates/dsr-cache/src/params.rs` | `Magic` and `Bullet` rows |
| `crates/dsr-cache/src/casts.rs` (new) | `ROLES`, `timing`, `extract`, `survey` |
| `crates/dsr-cache/src/cache.rs`, `main.rs` | helpers made `pub(crate)`, the `casts` / `casts-survey` commands, `extract` calls casts |

---

### Task 1: Class rows and class-aware abilities (S1)

The Warrior moves into a class row; the Mage row and its four abilities exist as data (`usable: false`). Nothing the user can see changes.

**Files:**
- Create: `crates/wow-world/src/class.rs`
- Modify: `crates/wow-world/src/lib.rs`, `crates/wow-world/src/equipment.rs:246-247,497-528,805-815`, `crates/wow-world/src/select.rs:262-267`, `crates/wow-world/src/abilities.rs` (whole file), `crates/wow-souls/src/abilities/mod.rs`, `crates/wow-souls/src/abilities/ui.rs:204`, `crates/wow-souls/src/abilities/moves.rs:61-68`
- Test: the same files' `#[cfg(test)]` modules

**Interfaces:**
- Produces: `wow_world::class::{Power, ClassDef, WARRIOR, MAGE, CLASSES, def(u8) -> Option<&'static ClassDef>, trainer_of(u32) -> Option<&'static ClassDef>, of_spell(u32) -> Option<&'static ClassDef>}`; `wow_world::equipment::{CLASS_MAGE, HUMAN_MAGE_KIT}`; `wow_world::abilities::Ability { power: Power, cost: u32, .. }` (the `rage` field is renamed `cost`), `abilities::known(learned, class: u8)`, `abilities::Effect::{Fireball, Frostbolt, FireBlast, FrostArmor}`; `wow_souls::abilities::Cast::Spell(u32)`, `abilities::slots_for(learned, class: u8)`.

- [ ] **Step 1: Write `class.rs` with its tests**

Create `crates/wow-world/src/class.rs`:

```rust
//! A class is a data row (spec `docs/superpowers/specs/2026-10-14-mage-class-design.md`, section 1):
//! what it spends to use its abilities, which abilities it has and in what order they are learned, the
//! trainer that teaches them and what a new human of the class wears. The abilities themselves are in
//! [`crate::abilities`]; the rest of the game asks this table which of them are a class's.

use crate::equipment::{Slot, CLASS_MAGE, CLASS_WARRIOR, HUMAN_MAGE_KIT, HUMAN_WARRIOR_KIT};

/// What a class spends to use an ability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    /// Builds from blows dealt and taken, fades away out of the fight.
    Rage,
    /// A pool from the class tables and Intellect that trickles back with Spirit.
    Mana,
}

impl Power {
    /// The word the refusals and the cards use.
    pub fn word(self) -> &'static str {
        match self {
            Power::Rage => "rage",
            Power::Mana => "mana",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ClassDef {
    /// The `ChrClasses` id.
    pub id: u8,
    pub name: &'static str,
    pub power: Power,
    /// The class trainer's creature entry.
    pub trainer: u32,
    /// The spell ids of the class's abilities, in the order they are learned.
    pub spells: &'static [u32],
    /// What a new human of the class wears.
    pub kit: &'static [(Slot, u32)],
}

pub const WARRIOR: ClassDef = ClassDef {
    id: CLASS_WARRIOR,
    name: "Warrior",
    power: Power::Rage,
    // Llane Beshere.
    trainer: 911,
    // Charge, Battle Shout, Thunder Clap.
    spells: &[100, 6673, 6343],
    kit: &HUMAN_WARRIOR_KIT,
};

pub const MAGE: ClassDef = ClassDef {
    id: CLASS_MAGE,
    name: "Mage",
    power: Power::Mana,
    // Khelden Bremen, `creature_template` 198, `trainer_id` 5.
    trainer: 198,
    // Fireball, Frost Armor (known from the start), Frostbolt (lvl 4), Fire Blast (lvl 6).
    spells: &[133, 168, 116, 2136],
    kit: &HUMAN_MAGE_KIT,
};

/// The classes the game can play.
pub static CLASSES: [ClassDef; 2] = [WARRIOR, MAGE];

/// The row of a class id; `None` for a class the game cannot play yet.
pub fn def(class: u8) -> Option<&'static ClassDef> {
    CLASSES.iter().find(|c| c.id == class)
}

/// The class a trainer (a creature entry) teaches.
pub fn trainer_of(npc: u32) -> Option<&'static ClassDef> {
    CLASSES.iter().find(|c| c.trainer == npc)
}

/// The class an ability (a spell id) belongs to.
pub fn of_spell(spell: u32) -> Option<&'static ClassDef> {
    CLASSES.iter().find(|c| c.spells.contains(&spell))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warrior_is_row_one_and_mage_row_two() {
        let w = def(1).unwrap();
        assert_eq!((w.name, w.power, w.trainer), ("Warrior", Power::Rage, 911));
        assert_eq!(w.spells, &[100, 6673, 6343], "Charge, Battle Shout, Thunder Clap");
        assert_eq!(w.kit.len(), 5);
        let m = def(8).unwrap();
        assert_eq!((m.name, m.power, m.trainer), ("Mage", Power::Mana, 198));
        assert_eq!(
            m.spells,
            &[133, 168, 116, 2136],
            "Fireball, Frost Armor, Frostbolt, Fire Blast"
        );
        assert_eq!(m.kit.len(), 5);
        assert!(def(2).is_none() && def(0).is_none(), "paladins are not playable yet");
    }

    #[test]
    fn a_trainer_and_a_spell_know_their_class() {
        assert_eq!(trainer_of(911).map(|c| c.id), Some(1));
        assert_eq!(trainer_of(198).map(|c| c.id), Some(8));
        assert!(trainer_of(1).is_none());
        assert_eq!(of_spell(6673).map(|c| c.id), Some(1));
        assert_eq!(of_spell(116).map(|c| c.id), Some(8));
        assert!(of_spell(78).is_none(), "no Heroic Strike");
    }

    #[test]
    fn the_kits_are_the_vanilla_ones() {
        assert!(MAGE.kit.contains(&(Slot::MainHand, 35)), "Bent Staff");
        assert!(MAGE.kit.contains(&(Slot::Chest, 56)), "Apprentice's Robe");
        assert!(MAGE.kit.contains(&(Slot::Legs, 1395)), "Apprentice's Pants");
        assert!(MAGE.kit.contains(&(Slot::Feet, 55)), "Apprentice's Boots");
        assert!(MAGE.kit.contains(&(Slot::Shirt, 6096)), "Apprentice's Shirt");
        assert!(WARRIOR.kit.contains(&(Slot::MainHand, 25)), "Worn Shortsword");
    }
}
```

- [ ] **Step 2: Register the module and add the mage kit**

In `crates/wow-world/src/lib.rs` add `pub mod class;` after `pub mod attributes;` (alphabetical: `abilities`, `attributes`, `class`, `db`).

In `crates/wow-world/src/equipment.rs`, after `pub const CLASS_WARRIOR: u8 = 1;` (line 247) add:

```rust
pub const CLASS_MAGE: u8 = 8;
```

After `HUMAN_WARRIOR_KIT` (ends line 505) add:

```rust
/// What a new human mage wears: vMaNGOS `playercreateinfo_item` for race 1, class 8, the pieces that go
/// on a slot (two Refreshing Spring Water, four Darnassian Bleu and the hearthstone go in a bag).
pub const HUMAN_MAGE_KIT: [(Slot, u32); 5] = [
    (Slot::MainHand, 35), // Bent Staff, two-handed staff 3-5, 2.9 s
    (Slot::Shirt, 6096),  // Apprentice's Shirt
    (Slot::Chest, 56),    // Apprentice's Robe
    (Slot::Legs, 1395),   // Apprentice's Pants
    (Slot::Feet, 55),     // Apprentice's Boots
];
```

Replace `start_kit` (lines 521-528):

```rust
/// The kit a new character of this race and class starts in; empty for the ones the game cannot make.
pub fn start_kit(race: u8, class: u8) -> &'static [(Slot, u32)] {
    if race != RACE_HUMAN {
        return &[][..];
    }
    crate::class::def(class).map_or(&[][..], |c| c.kit)
}
```

In the test `a_human_warrior_starts_in_the_vanilla_kit` (line 807) replace the line `assert!(start_kit(RACE_HUMAN, 8).is_empty());` with:

```rust
        assert_eq!(start_kit(RACE_HUMAN, CLASS_MAGE).len(), 5, "the mage has a kit");
        assert!(start_kit(RACE_HUMAN, 5).is_empty(), "the priest has none yet");
```

- [ ] **Step 3: Item selection includes every class kit**

In `crates/wow-world/src/select.rs` replace lines 262-267:

```rust
    item_ids.extend(
        crate::class::CLASSES
            .iter()
            .flat_map(|c| c.kit.iter())
            .chain(&crate::equipment::TRIAL_GEAR)
            .map(|&(_, id)| id),
    );
```

and update the doc comment at line 24 (`[`crate::equipment::HUMAN_WARRIOR_KIT`]`) to say "every class kit ([`crate::class::CLASSES`])".

- [ ] **Step 4: Replace `crates/wow-world/src/abilities.rs`**

Write the whole file (the Warrior's three rows keep every number; `rage` becomes `power` + `cost`):

```rust
//! The abilities of the classes the game plays, and what their trainers ask for them. Pure data and
//! rules; the game's rage and mana, hotbar and the trainer's screen draw them.
//!
//! **The Warrior's** set is the one the user chose: **Charge, Battle Shout and Thunder Clap**, learned from
//! Llane Beshere (`npc_trainer_template` 22, build 5875), Charge first. Heroic Strike (it does nothing by
//! itself, it only makes the next blow harder) and Rend are not in the game. The vMaNGOS trainer asks
//! level 1 for Battle Shout, 4 for Charge and 6 for Thunder Clap; here Charge is the first thing a
//! warrior learns (level 1) and Thunder Clap asks 4, because level 6 is out of reach in the valley. The
//! prices are the database's (10 copper for the shout, 100 for the others), paid in souls
//! ([`crate::shop::SOULS_PER_COPPER`]). Rage costs are the spells' own (`manaCost` / 10 of a rage spell).
//!
//! **The Mage's** set is the user's too (2026-10-14): **Fireball, Frost Armor, Frostbolt and Fire Blast**,
//! rank 1. Fireball and Frost Armor are known from the start (`playercreateinfo_spell`), Frostbolt (lvl 4)
//! and Fire Blast (lvl 6) are taught by Khelden Bremen (`npc_trainer_template` 5) for 100 copper each.
//! Mana costs are the spells' own `manaCost`.
//!
//! `usable` says whether the game does something when the ability is used; an ability that is not usable
//! cannot be learned (it would cost souls and do nothing). Which abilities are whose, and in what order
//! they are learned, is [`crate::class`].

use std::collections::BTreeSet;

use crate::class::Power;
use crate::shop;

/// The highest rage a warrior holds.
pub const RAGE_MAX: u32 = 100;

/// What using the ability does; the game maps each to its effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Every blow hits harder for a while.
    BattleShout,
    /// A rush at the enemy that stuns it.
    Charge,
    /// Blasts what stands near.
    ThunderClap,
    /// A fire missile that burns.
    Fireball,
    /// A frost missile that slows.
    Frostbolt,
    /// An instant blast at a target in range.
    FireBlast,
    /// More armor for half an hour.
    FrostArmor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ability {
    /// The spell id of the ability (rank 1).
    pub spell: u32,
    pub name: &'static str,
    pub rank: u8,
    /// The level the trainer asks.
    pub level: u32,
    /// The trainer's price in copper (0: known from the start).
    pub copper: u32,
    /// What it is paid in, and how much.
    pub power: Power,
    pub cost: u32,
    /// The client's `SpellIcon` id and the icon it names (`Interface\Icons\<icon>`).
    pub icon: &'static str,
    pub effect: Effect,
    pub usable: bool,
    /// What the card says (WoW's own sense, in the game's numbers).
    pub text: &'static str,
}

impl Ability {
    /// The price in souls.
    pub fn souls(&self) -> u32 {
        shop::souls_for_copper(self.copper)
    }
}

/// Every ability of every class, a class's own in the order of its row ([`crate::class::ClassDef::spells`]).
pub const ALL: [Ability; 7] = [
    Ability {
        spell: 100,
        name: "Charge",
        rank: 1,
        level: 1,
        copper: 100,
        power: Power::Rage,
        cost: 0,
        icon: "Ability_Warrior_Charge",
        effect: Effect::Charge,
        usable: true,
        text: "Charge an enemy 5 to 25 yards away: you rush to it, gain 9 rage and it is stunned for 1.5 seconds. 12 second cooldown.",
    },
    Ability {
        spell: 6673,
        name: "Battle Shout",
        rank: 1,
        level: 1,
        copper: 10,
        power: Power::Rage,
        cost: 20,
        icon: "Ability_Warrior_BattleShout",
        effect: Effect::BattleShout,
        usable: true,
        text: "You shout: your blows do 15% more damage for 2 minutes.",
    },
    Ability {
        spell: 6343,
        name: "Thunder Clap",
        rank: 1,
        level: 4,
        copper: 100,
        power: Power::Rage,
        cost: 20,
        icon: "Spell_Nature_ThunderClap",
        effect: Effect::ThunderClap,
        usable: true,
        text: "Blasts every enemy within 5 yards with thunder: 30 damage, and they move and recover 40% slower for 10 seconds. 5 second cooldown.",
    },
    Ability {
        spell: 133,
        name: "Fireball",
        rank: 1,
        level: 1,
        copper: 0,
        power: Power::Mana,
        cost: 30,
        icon: "Spell_Fire_FlameBolt",
        effect: Effect::Fireball,
        usable: false,
        text: "Hurls a fiery ball that causes 14 to 22 fire damage and an additional 2 fire damage over 4 seconds.",
    },
    Ability {
        spell: 168,
        name: "Frost Armor",
        rank: 1,
        level: 1,
        copper: 0,
        power: Power::Mana,
        cost: 60,
        icon: "Spell_Frost_FrostArmor02",
        effect: Effect::FrostArmor,
        usable: false,
        text: "Covers you in a layer of frost: your armor is increased by 30 for 30 minutes.",
    },
    Ability {
        spell: 116,
        name: "Frostbolt",
        rank: 1,
        level: 4,
        copper: 100,
        power: Power::Mana,
        cost: 25,
        icon: "Spell_Frost_FrostBolt02",
        effect: Effect::Frostbolt,
        usable: false,
        text: "Launches a bolt of frost that causes 18 to 20 frost damage and slows the enemy's movement by 42% for 5 seconds.",
    },
    Ability {
        spell: 2136,
        name: "Fire Blast",
        rank: 1,
        level: 6,
        copper: 100,
        power: Power::Mana,
        cost: 40,
        icon: "Spell_Fire_Fireball",
        effect: Effect::FireBlast,
        usable: false,
        text: "Blasts the enemy within 20 yards for 24 to 32 fire damage. 8 second cooldown.",
    },
];

/// The warrior's trainer: Llane Beshere (the same number as [`crate::class::WARRIOR`]'s).
pub const WARRIOR_TRAINER: u32 = 911;

pub fn find(spell: u32) -> Option<&'static Ability> {
    ALL.iter().find(|a| a.spell == spell)
}

/// What a trainer sells: the abilities of the class it teaches that have a price.
pub fn trainer_list(npc: u32) -> Vec<&'static Ability> {
    let Some(class) = crate::class::trainer_of(npc) else {
        return Vec::new();
    };
    class
        .spells
        .iter()
        .filter_map(|&s| find(s))
        .filter(|a| a.copper > 0)
        .collect()
}

/// What the character knows: the starting abilities and what was learned.
pub fn knows(learned: &BTreeSet<u32>, ability: &Ability) -> bool {
    ability.copper == 0 || learned.contains(&ability.spell)
}

/// The abilities a character of `class` knows, in the class's learning order.
pub fn known(learned: &BTreeSet<u32>, class: u8) -> Vec<&'static Ability> {
    let Some(class) = crate::class::def(class) else {
        return Vec::new();
    };
    class
        .spells
        .iter()
        .filter_map(|&s| find(s))
        .filter(|a| knows(learned, a))
        .collect()
}

/// Why the trainer will not teach it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Known,
    Level(u32),
    NotEnoughSouls {
        need: u32,
        have: u32,
    },
    /// The game does not do anything with it yet.
    NotYet,
}

impl Refusal {
    pub fn message(self) -> String {
        match self {
            Refusal::Known => "You already know that.".into(),
            Refusal::Level(n) => format!("You must reach level {n} to learn that."),
            Refusal::NotEnoughSouls { need, have } => {
                format!("Not enough souls: it costs {need}, you carry {have}.")
            }
            Refusal::NotYet => "The trainer cannot teach that yet.".into(),
        }
    }
}

/// The trainer's verdict on teaching `ability` to a character of `level` carrying `souls`.
pub fn can_learn(
    learned: &BTreeSet<u32>,
    ability: &Ability,
    level: u32,
    souls: u32,
) -> Result<(), Refusal> {
    if knows(learned, ability) {
        return Err(Refusal::Known);
    }
    if !ability.usable {
        return Err(Refusal::NotYet);
    }
    if level < ability.level {
        return Err(Refusal::Level(ability.level));
    }
    let need = ability.souls();
    if souls < need {
        return Err(Refusal::NotEnoughSouls { need, have: souls });
    }
    Ok(())
}

/// Teaches it: takes the souls and adds the ability; returns the price. Nothing changes on a refusal.
pub fn learn(
    learned: &mut BTreeSet<u32>,
    souls: &mut u32,
    ability: &Ability,
    level: u32,
) -> Result<u32, Refusal> {
    can_learn(learned, ability, level, *souls)?;
    let cost = ability.souls();
    *souls -= cost;
    learned.insert(ability.spell);
    Ok(cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shout() -> &'static Ability {
        find(6673).unwrap()
    }
    fn charge() -> &'static Ability {
        find(100).unwrap()
    }

    #[test]
    fn nothing_is_known_at_the_start_and_the_trainer_sells_charge_first() {
        let none = BTreeSet::new();
        assert!(
            known(&none, 1).is_empty(),
            "no Heroic Strike, nothing for free"
        );
        assert!(!knows(&none, charge()));
        let list = trainer_list(WARRIOR_TRAINER);
        assert_eq!(
            list.iter().map(|a| a.name).collect::<Vec<_>>(),
            vec!["Charge", "Battle Shout", "Thunder Clap"],
            "the set the user chose, in the order it is learned"
        );
        assert_eq!(
            list.iter().map(|a| a.souls()).collect::<Vec<_>>(),
            vec![400, 40, 400]
        );
        assert!(
            ALL.iter()
                .filter(|a| a.power == Power::Rage)
                .all(|a| a.usable),
            "every warrior ability works"
        );
        assert!(trainer_list(1).is_empty());
    }

    #[test]
    fn the_mage_knows_fireball_and_frost_armor_and_learns_the_other_two() {
        let none = BTreeSet::new();
        assert_eq!(
            known(&none, 8).iter().map(|a| a.name).collect::<Vec<_>>(),
            vec!["Fireball", "Frost Armor"],
            "known from the start, in the class's order"
        );
        let list = trainer_list(198);
        assert_eq!(
            list.iter().map(|a| a.name).collect::<Vec<_>>(),
            vec!["Frostbolt", "Fire Blast"]
        );
        assert_eq!(
            list.iter().map(|a| (a.level, a.souls())).collect::<Vec<_>>(),
            vec![(4, 400), (6, 400)]
        );
        // Nothing is usable until the game does something with it.
        assert!(ALL
            .iter()
            .filter(|a| a.power == Power::Mana)
            .all(|a| !a.usable));
        // A class never lists another's abilities.
        assert!(known(&none, 1).iter().all(|a| a.power == Power::Rage));
        assert!(known(&none, 8).iter().all(|a| a.power == Power::Mana));
    }

    #[test]
    fn charge_is_the_first_thing_a_new_warrior_can_learn() {
        let learned = BTreeSet::new();
        assert_eq!(can_learn(&learned, charge(), 1, 400), Ok(()));
        assert_eq!(
            can_learn(&learned, charge(), 1, 399),
            Err(Refusal::NotEnoughSouls {
                need: 400,
                have: 399
            })
        );
        // Thunder Clap waits for level 4, which the valley can reach.
        let clap = find(6343).unwrap();
        assert_eq!(can_learn(&learned, clap, 3, 999), Err(Refusal::Level(4)));
        assert_eq!(can_learn(&learned, clap, 4, 999), Ok(()));
    }

    #[test]
    fn learning_needs_the_level_the_souls_and_a_usable_ability() {
        let mut learned = BTreeSet::new();
        let mut souls = 30;
        assert_eq!(
            learn(&mut learned, &mut souls, shout(), 1),
            Err(Refusal::NotEnoughSouls { need: 40, have: 30 })
        );
        souls = 100;
        assert_eq!(learn(&mut learned, &mut souls, shout(), 1), Ok(40));
        assert_eq!((souls, learned.contains(&6673)), (60, true));
        assert_eq!(
            learn(&mut learned, &mut souls, shout(), 1),
            Err(Refusal::Known)
        );
        // An ability the game cannot do is refused and costs nothing.
        let mut broken = *charge();
        broken.usable = false;
        assert_eq!(
            learn(&mut learned, &mut souls, &broken, 9),
            Err(Refusal::NotYet)
        );
        assert_eq!((souls, learned.len()), (60, 1), "a refusal changes nothing");
    }

    #[test]
    fn the_level_gate_speaks_in_wows_words() {
        let mut ability = *shout();
        ability.level = 4;
        let learned = BTreeSet::new();
        assert_eq!(
            can_learn(&learned, &ability, 3, 999),
            Err(Refusal::Level(4))
        );
        assert_eq!(
            Refusal::Level(4).message(),
            "You must reach level 4 to learn that."
        );
        assert_eq!(can_learn(&learned, &ability, 4, 999), Ok(()));
    }

    #[test]
    fn the_costs_are_the_spells_own() {
        let cost = |s: u32| find(s).unwrap().cost;
        assert_eq!((cost(100), cost(6673), cost(6343)), (0, 20, 20));
        assert_eq!(
            (cost(133), cost(168), cost(116), cost(2136)),
            (30, 60, 25, 40),
            "Fireball, Frost Armor, Frostbolt, Fire Blast mana"
        );
        assert!(ALL
            .iter()
            .filter(|a| a.power == Power::Rage)
            .all(|a| a.cost <= RAGE_MAX));
        assert!(
            find(78).is_none() && find(772).is_none(),
            "Heroic Strike and Rend are not in the game"
        );
    }
}
```

- [ ] **Step 5: Run the wow-world tests**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world abilities class equipment 2>&1 | tail -30`
Expected: PASS (the `class`, `abilities`, `equipment` tests). If `select` tests count items, they may now differ: fix the expected counts only if the failing assertion is about the number of selected items (the mage kit adds 5 rows), and say so in the log.

- [ ] **Step 6: Make the game compile on the renamed field and the new signatures**

In `crates/wow-souls/src/abilities/moves.rs` replace `cooldown` (lines 61-68):

```rust
/// The cooldown of an ability (s): Charge 12, Thunder Clap 5, Fire Blast 8, the rest none.
pub fn cooldown(effect: Effect) -> f32 {
    match effect {
        Effect::Charge => 12.0,
        Effect::ThunderClap => 5.0,
        Effect::FireBlast => 8.0,
        Effect::BattleShout | Effect::Fireball | Effect::Frostbolt | Effect::FrostArmor => 0.0,
    }
}
```

In `crates/wow-souls/src/abilities/mod.rs`:

1. `Cast` enum gains a variant:

```rust
pub enum Cast {
    Shout,
    Charge(Entity),
    Clap,
    /// A mage's spell: the world is asked to cast it (`spells`, a later task).
    Spell(u32),
}
```

2. In `activate`, rename the rage reads and add the spell arm:

```rust
    if rage.cur < ability.cost as f32 {
        return Err(Refusal::NotEnoughRage(ability.cost));
    }
    let cast = match ability.effect {
        Effect::BattleShout => Cast::Shout,
        Effect::Charge => Cast::Charge(aim.unwrap_or(Err(Refusal::NoTarget))?),
        Effect::ThunderClap => Cast::Clap,
        Effect::Fireball | Effect::Frostbolt | Effect::FireBlast | Effect::FrostArmor => {
            Cast::Spell(ability.spell)
        }
    };
    rage.cur -= ability.cost as f32;
```

3. In `input`, the `match cast` gets an arm after `Cast::Clap`:

```rust
                    // A mage's casts start in a later task (`spells`); until then they are not usable.
                    Cast::Spell(_) => {}
```

4. `slots_for`:

```rust
/// The slots for what is learned: the known and usable abilities of `class` in list order, four at most.
pub fn slots_for(learned: &std::collections::BTreeSet<u32>, class: u8) -> Vec<u32> {
    abilities::known(learned, class)
        .into_iter()
        .filter(|a| a.usable)
        .map(|a| a.spell)
        .take(4)
        .collect()
}
```

5. `refit` takes the player's class from the quest game:

```rust
/// The bar follows what is learned.
fn refit(owned: Res<Owned>, game: Option<Res<QuestGame>>, mut bar: ResMut<Hotbar>) {
    let class = game.map_or(1, |g| g.player.class as u8);
    let slots = slots_for(&owned.progress.abilities, class);
    if bar.slots != slots {
        bar.selected = bar.selected.min(slots.len().saturating_sub(1));
        bar.slots = slots;
    }
}
```

and add `use crate::quests::{QuestGame, Toasts};` (replace the existing `use crate::quests::Toasts;`).

6. `card`: the cost line:

```rust
    let cost = if a.cost > 0 {
        let word = match a.power {
            wow_world::class::Power::Rage => "Rage",
            wow_world::class::Power::Mana => "Mana",
        };
        format!("{} {word}", a.cost)
    } else {
        format!("No {}", a.power.word())
    };
```

7. Tests in the same file: `slots_for(&learned)` → `slots_for(&learned, 1)` (three calls in `the_bar_holds_the_known_usable_abilities_in_list_order`); `abilities::ALL` loop in `every_ability_has_an_icon_in_the_install` stays.

In `crates/wow-souls/src/abilities/ui.rs` line 204 replace `a.rage` with `a.cost`.

- [ ] **Step 7: Add the class-isolation tests to the game**

In `crates/wow-souls/src/abilities/mod.rs` tests add:

```rust
    #[test]
    fn a_mage_gets_no_bar_slot_until_the_game_can_cast_its_spells() {
        let mut learned = BTreeSet::new();
        learned.insert(116);
        assert!(
            slots_for(&learned, 8).is_empty(),
            "the mage's abilities are not usable yet"
        );
        assert_eq!(slots_for(&learned, 1), Vec::<u32>::new(), "and a warrior never lists them");
    }

    #[test]
    fn a_mage_ability_is_refused_as_not_yet_and_costs_nothing() {
        let (mut rage, mut buffs, mut cd) = (rage_of(100.0), Buffs::default(), Cooldowns::default());
        assert_eq!(
            activate(&mut rage, &mut buffs, &mut cd, ability(133), None),
            Err(Refusal::NotYet)
        );
        assert_eq!((rage.cur, buffs), (100.0, Buffs::default()));
    }

    #[test]
    fn the_card_names_mana_for_a_mage_spell() {
        let c = card(116).unwrap();
        assert!(c.tooltip.starts_with("Rank 1   25 Mana\n"), "{}", c.tooltip);
        assert_eq!(c.versus, "Requires level 4");
        assert_eq!(card(133).unwrap().versus, "", "Fireball is known from the start");
    }
```

- [ ] **Step 8: Run the whole gate**

Run:
```bash
cd /c/Spel/wow-souls && cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check && cargo clippy -j 4 --workspace --all-targets -- -D warnings && DSR_DIR="C:/Spel/Dark Souls Remastered" WOW_DATA="C:/Spel/World of Warcraft 1.12/Data" cargo test -j 4 --workspace 2>&1 | tail -25
```
Expected: all green; counts are the last green counts plus the new tests (wow-world +6, wow-souls +3). If the icon install test fails on an icon name (`Spell_Fire_FlameBolt`, `Spell_Frost_FrostBolt02`, `Spell_Fire_Fireball`, `Spell_Frost_FrostArmor02`), find the right file with `grep -i -a -o 'Spell_Frost_Frost[A-Za-z0-9]*' ` over `benilla-extract list` output for `Interface\Icons` and fix the name in `ALL`.

- [ ] **Step 9: Rebuild the world manifest so the mage's kit items exist in it**

The kit items (35, 6096, 56, 1395, 55) must be in `cache/world/northshire.txt`.

```bash
cd /c/Spel/wow-souls && cp cache/world/northshire.txt cache/world/northshire.pre-mage.bak && cargo run -j 4 -p wow-world --bin wow-souls-world -- import 2>&1 | tail -15
```
(If the importer wants `--wow <Data dir>`, read `crates/wow-world/src/main.rs` usage and pass `"C:/Spel/World of Warcraft 1.12/Data"`.) Then:

```bash
diff <(grep -c '^item ' cache/world/northshire.pre-mage.bak) <(grep -c '^item ' cache/world/northshire.txt); grep -E '^item (35|56|1395|55|6096)\b' cache/world/northshire.txt | cut -c1-80
```
Expected: the item count rises by up to 5 and all five rows are present (an item already selected for another reason does not add).

- [ ] **Step 10: Gate and log**

Re-run the Step 8 gate once more (the manifest changed the item tables). Write `docs/log/2026-10-14-mage-s1.md` (what changed, the new test counts) and set HANDOVER item 1 to "S1 done: class rows, Warrior unchanged; Mage abilities exist as unusable data". No commit.

---

### Task 2: A mage can be made and plays with a mana bar (S2)

**Files:**
- Modify: `crates/wow-world/src/quest/mod.rs:60-112`, `crates/wow-world/src/attributes.rs`, `crates/wow-souls/src/character.rs`, `crates/wow-souls/src/quests/mod.rs:328-360`, `crates/wow-souls/src/abilities/mod.rs`, `crates/wow-souls/src/abilities/ui.rs`, `crates/wow-souls/src/level.rs`
- Test: the same files' test modules

**Interfaces:**
- Consumes (Task 1): `class::{Power, def}`, `Ability { power, cost }`, `abilities::slots_for(learned, class)`, `Cast::Spell`.
- Produces: `wow_world::quest::{Class::from_id(u8) -> Option<Class>, Player::of_class(&str, Class, u32) -> Player}`; `wow_world::attributes::{intellect_mana(u32) -> f32, INTELLECT_FREE, MANA_PER_INTELLECT, MANA_OFFSET, Character.max_mana: f32}`; game `abilities::{Mana { cur, max, quiet, spirit }, Mana::sync(max, spirit), Mana::spend(f32), Mana::tick(dt), Mana::refill(), MANA_QUIET, REGEN_PER_SPIRIT, ActiveClass { id, power }, Refusal::NotEnough(Power)}`; `activate(rage, mana, buffs, cooldowns, ability, aim)`; `pub fn bar_of(power, &Rage, &Mana) -> (f32, f32)` in `abilities::ui`.

- [ ] **Step 1: Failing test for the mana formula and the mage's numbers**

In `crates/wow-world/src/attributes.rs` test module (it has the `tables()` fixture for the warrior), add:

```rust
    #[test]
    fn intellect_gives_a_mana_point_each_for_twenty_then_fifteen() {
        assert_eq!(intellect_mana(0), 0.0);
        assert_eq!(intellect_mana(20), 20.0);
        assert_eq!(intellect_mana(23), 20.0 + 3.0 * 15.0);
    }

    #[test]
    fn a_human_mage_at_level_one_has_165_mana_and_52_health() {
        let levels = [LevelStat {
            class: 8,
            level: 1,
            strength: 20,
            agility: 20,
            stamina: 20,
            intellect: 23,
            spirit: 22,
        }];
        let classes = [ClassStat {
            class: 8,
            level: 1,
            base_hp: 32,
            base_mana: 100,
        }];
        let c = character(&levels, &classes, 8, 1, &Gear::default(), &[]).unwrap();
        assert_eq!(c.max_mana, 165.0, "100 base + 20 + 3 x 15");
        assert_eq!(c.max_health, 52.0, "32 base + 20 from Stamina");
        // A class with no mana in its table has none, whatever its Intellect.
        let warrior = [ClassStat {
            class: 1,
            level: 1,
            base_hp: 20,
            base_mana: 0,
        }];
        let w_levels = [LevelStat {
            class: 1,
            level: 1,
            strength: 23,
            agility: 20,
            stamina: 22,
            intellect: 20,
            spirit: 20,
        }];
        let w = character(&w_levels, &warrior, 1, 1, &Gear::default(), &[]).unwrap();
        assert_eq!(w.max_mana, 0.0);
        assert_eq!(w.max_health, 60.0, "the warrior is unchanged");
    }
```
(`Gear::default()` and the `use` lines exist at the top of the test module: `use crate::equipment::{inv, tests::item, Gear, Slot}; use crate::model::{ClassStat, Item, LevelStat};`. If `LevelStat` has other fields, copy them from the `tables()` fixture.)

- [ ] **Step 2: Run to see it fail**

Run: `cd /c/Spel/wow-souls && cargo test -j 4 -p wow-world attributes 2>&1 | tail -15`
Expected: FAIL: "cannot find function `intellect_mana`" / "no field `max_mana`".

- [ ] **Step 3: Implement the mana formula**

In `crates/wow-world/src/attributes.rs`, after `ARMOR_PER_AGILITY`:

```rust
/// The first this many Intellect points give a mana point each; every point above gives
/// [`MANA_PER_INTELLECT`] (MaNGOS `GetManaBonusFromIntellect`).
pub const INTELLECT_FREE: u32 = 20;
pub const MANA_PER_INTELLECT: f32 = 15.0;
/// Added to every class's max mana that has any (0: WoW's own numbers; the lever if a level-1 mage's
/// 165 feels wrong, as [`HEALTH_OFFSET`] is for health).
pub const MANA_OFFSET: f32 = 0.0;

/// The mana Intellect gives.
pub fn intellect_mana(intellect: u32) -> f32 {
    let free = intellect.min(INTELLECT_FREE) as f32;
    let above = intellect.saturating_sub(INTELLECT_FREE) as f32;
    free + above * MANA_PER_INTELLECT
}
```

Add `pub max_mana: f32,` to `Character` (after `max_health`), set it in `character()` before the `Some(Character { .. })`:

```rust
    // Only a class whose table has mana has a pool (a warrior's Intellect buys nothing).
    let max_mana = if c.base_mana > 0 {
        c.base_mana as f32 + intellect_mana(total.intellect) + MANA_OFFSET
    } else {
        0.0
    };
```
and `max_mana,` in the struct literal. In `sheet()` change the second line to include mana when there is any:

```rust
    let mana = if c.max_mana > 0.0 {
        format!("   Mana {}", c.max_mana.round() as i64)
    } else {
        String::new()
    };
    format!(
        "{}   {}   {}\n{}   {}   Health {}{mana}",
        /* the same arguments as before */
    )
```
Update the existing `sheet` test expectation only if it builds a character with mana (the warrior's has none: its string is unchanged).

- [ ] **Step 4: Run to see it pass**

Run: `cargo test -j 4 -p wow-world attributes 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 5: The quest player carries the class (failing test, then code)**

In `crates/wow-world/src/quest/mod.rs` test area (or a new `#[cfg(test)] mod class_tests` at the end of the file) add:

```rust
#[cfg(test)]
mod class_tests {
    use super::*;

    #[test]
    fn a_class_id_names_its_class_and_the_player_carries_it() {
        assert_eq!(Class::from_id(1), Some(Class::Warrior));
        assert_eq!(Class::from_id(8), Some(Class::Mage));
        assert_eq!(Class::from_id(6), None, "6 is not a 1.12 class");
        let p = Player::of_class("Merlin", Class::Mage, 3);
        assert_eq!((p.class, p.race, p.male, p.level), (Class::Mage, Race::Human, true, 3));
        assert_eq!(p.class as u32, 8, "the class tables are keyed by this id");
        assert_eq!(Player::warrior("Jacob", 1).class, Class::Warrior);
    }
}
```

Run: `cargo test -j 4 -p wow-world class_tests 2>&1 | tail` → FAIL (no `from_id`). Implement, in the `impl Class` block next to `mask`:

```rust
    /// The class with this `ChrClasses` id.
    pub fn from_id(id: u8) -> Option<Class> {
        [
            Class::Warrior,
            Class::Paladin,
            Class::Hunter,
            Class::Rogue,
            Class::Priest,
            Class::Shaman,
            Class::Mage,
            Class::Warlock,
            Class::Druid,
        ]
        .into_iter()
        .find(|c| *c as u8 == id)
    }
```
and in `impl Player`:

```rust
    /// A Human male of `class` (the game plays only humans, male).
    pub fn of_class(name: &str, class: Class, level: u32) -> Player {
        Player {
            name: name.to_string(),
            male: true,
            class,
            race: Race::Human,
            level,
        }
    }
```
Run again → PASS.

- [ ] **Step 6: The game reads the chosen class**

In `crates/wow-souls/src/quests/mod.rs` replace lines 328-336 (the `character` / `name` block) and line 358 so the character file gives the class too:

```rust
    // The name and the class: the environment, else the created character's, else the old save's.
    let character = crate::character::load(&crate::character::save_path())
        .ok()
        .flatten();
    let class = character
        .as_ref()
        .and_then(|c| wow_world::quest::Class::from_id(c.class.id()))
        .unwrap_or(wow_world::quest::Class::Warrior);
    let name = std::env::var("WOWSOULS_NAME")
        .ok()
        .filter(|n| !n.trim().is_empty())
        .or(character.map(|c| c.name))
        .or_else(|| (!save.name.is_empty()).then(|| save.name.clone()))
        .unwrap_or_else(|| "Adventurer".to_string());
```
and `player: Player::of_class(&name, class, level),`.

In `crates/wow-souls/src/character.rs` replace `Class::enabled` and the module doc line 3-4:

```rust
    /// The classes the game can play: the Warrior and the Mage.
    pub fn enabled(self) -> bool {
        matches!(self, Class::Warrior | Class::Mage)
    }
```
Doc: "Only a Human male Warrior or Mage can be made for now". Update the test (line ~384) to:

```rust
    #[test]
    fn only_a_human_male_warrior_or_mage_can_be_made_yet() {
        let c = Character::human_warrior("Jacob");
        assert!(c.playable());
        let mage = Character {
            class: Class::Mage,
            ..c.clone()
        };
        assert!(mage.playable());
        for race in Race::ALL.into_iter().filter(|r| *r != Race::Human) {
            assert!(!race.enabled(), "{race:?}");
        }
        for class in Class::ALL
            .into_iter()
            .filter(|c| !matches!(c, Class::Warrior | Class::Mage))
        {
            assert!(!class.enabled(), "{class:?}");
        }
        assert!(!Sex::Female.enabled());
        let elf = Character {
            race: Race::NightElf,
            ..c
        };
        assert!(!elf.playable());
    }
```
(Keep whatever the original test's final assertions were; the point is Warrior and Mage playable, the rest not.)

- [ ] **Step 7: `Mana` and the class in the game (failing tests first)**

In `crates/wow-souls/src/abilities/mod.rs` tests add:

```rust
    #[test]
    fn the_pool_starts_full_and_a_longer_bar_is_not_a_refill() {
        let mut m = Mana::default();
        m.sync(165.0, 22.0);
        assert_eq!((m.cur, m.max, m.spirit), (165.0, 165.0, 22.0), "the first sync fills the pool");
        m.spend(60.0);
        assert_eq!(m.cur, 105.0);
        // A level-up lengthens the bar by 20: the pool moves by the same 20.
        m.sync(185.0, 23.0);
        assert_eq!((m.cur, m.max), (125.0, 185.0));
        // Gear that gave Intellect comes off: the pool clamps to the new maximum, never above, never below zero.
        m.sync(100.0, 22.0);
        assert_eq!((m.cur, m.max), (100.0, 100.0));
        m.cur = 5.0;
        m.sync(40.0, 22.0);
        assert_eq!(m.cur, 0.0, "5 - 60 clamps at zero");
    }

    #[test]
    fn mana_waits_five_seconds_after_a_cast_then_trickles_back_with_spirit() {
        let mut m = Mana::default();
        m.sync(100.0, 20.0);
        m.spend(50.0);
        m.tick(4.9);
        assert_eq!(m.cur, 50.0, "the five-second rule");
        m.tick(0.2);
        // 0.1 s past the rule: 0.07 per Spirit point per second.
        assert!(m.cur > 50.0 && m.cur < 50.5, "{}", m.cur);
        for _ in 0..100 {
            m.tick(1.0);
        }
        assert_eq!(m.cur, 100.0, "it stops at the maximum");
        // A cast starts the wait over; a new life is a full pool.
        m.spend(10.0);
        assert_eq!(m.quiet, 0.0);
        m.refill();
        assert_eq!(m.cur, 100.0);
    }

    #[test]
    fn a_spell_is_paid_in_its_own_power() {
        let (mut rage, mut mana) = (rage_of(100.0), Mana::default());
        mana.sync(100.0, 20.0);
        let (mut buffs, mut cd) = (Buffs::default(), Cooldowns::default());
        let mut usable = *ability(116); // Frostbolt, 25 mana
        usable.usable = true;
        assert_eq!(
            activate(&mut rage, &mut mana, &mut buffs, &mut cd, &usable, None),
            Ok(Cast::Spell(116))
        );
        assert_eq!((rage.cur, mana.cur), (100.0, 75.0), "mana paid, rage untouched");
        mana.cur = 20.0;
        assert_eq!(
            activate(&mut rage, &mut mana, &mut buffs, &mut cd, &usable, None),
            Err(Refusal::NotEnough(wow_world::class::Power::Mana))
        );
        assert_eq!(mana.cur, 20.0, "a refusal spends nothing");
        assert_eq!(
            Refusal::NotEnough(wow_world::class::Power::Mana).message(),
            "Not enough mana."
        );
    }

    #[test]
    fn a_fight_builds_rage_only_for_a_class_that_has_it() {
        assert!(builds_rage(wow_world::class::Power::Rage));
        assert!(!builds_rage(wow_world::class::Power::Mana));
    }
```

and update the existing tests for the new `activate` signature: every `activate(&mut rage, &mut buffs, &mut cd, ...)` becomes `activate(&mut rage, &mut Mana::default(), &mut buffs, &mut cd, ...)`, and `Refusal::NotEnoughRage(20)` becomes `Refusal::NotEnough(wow_world::class::Power::Rage)` (in `thunder_clap_costs_twenty_rage_and_five_seconds` and `the_refusals_speak_plainly`; the message stays "Not enough rage.").

Run: `cargo test -j 4 -p wow-souls abilities 2>&1 | tail -20` → FAIL (compile).

- [ ] **Step 8: Implement `Mana`, `ActiveClass`, the two-power `activate`**

In `crates/wow-souls/src/abilities/mod.rs`:

```rust
use wow_world::class::{self, Power};
```

Constants (next to `DEALT` / `TAKEN`):

```rust
/// Seconds after a cast before mana comes back (WoW's five-second rule).
pub const MANA_QUIET: f32 = 5.0;
/// Mana a second per point of Spirit once it comes back: a level-1 human mage (Spirit 22) regains
/// about 1.5 a second. Claude's first guess, tuned in play.
pub const REGEN_PER_SPIRIT: f32 = 0.07;
```

Resources:

```rust
/// A caster's mana pool and what refills it.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct Mana {
    pub cur: f32,
    pub max: f32,
    /// Seconds since the last cast.
    pub quiet: f32,
    /// The character's Spirit, which sets the trickle.
    pub spirit: f32,
    /// The pool was sized at least once: the first time it starts full.
    set: bool,
}

impl Mana {
    /// The tables and the gear say `max` and `spirit` now. The first call fills the pool (or keeps a
    /// positive `cur` a debug switch put there, clamped); later ones move the current value by the same
    /// difference as the maximum, so a longer bar is not a refill, and never leave it outside `0..=max`.
    pub fn sync(&mut self, max: f32, spirit: f32) {
        self.spirit = spirit;
        if self.set {
            self.cur = (self.cur + max - self.max).clamp(0.0, max);
        } else {
            self.set = true;
            self.cur = if self.cur > 0.0 { self.cur.min(max) } else { max };
        }
        self.max = max;
    }

    pub fn spend(&mut self, cost: f32) {
        self.cur = (self.cur - cost).max(0.0);
        self.quiet = 0.0;
    }

    /// `dt` seconds pass: after the quiet, Spirit gives mana back.
    pub fn tick(&mut self, dt: f32) {
        self.quiet += dt;
        if self.quiet >= MANA_QUIET && self.max > 0.0 {
            self.cur = (self.cur + REGEN_PER_SPIRIT * self.spirit * dt).min(self.max);
        }
    }

    /// A new life starts with a full pool.
    pub fn refill(&mut self) {
        self.cur = self.max;
        self.quiet = MANA_QUIET;
    }
}

/// Which class the character is and what it spends, from the quest game's player.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveClass {
    pub id: u8,
    pub power: Power,
}

impl Default for ActiveClass {
    fn default() -> Self {
        ActiveClass {
            id: class::WARRIOR.id,
            power: Power::Rage,
        }
    }
}

/// Rage builds from blows for a class that has rage; a mage's fight builds none.
pub fn builds_rage(power: Power) -> bool {
    power == Power::Rage
}

fn follow_class(game: Option<Res<QuestGame>>, mut active: ResMut<ActiveClass>) {
    let Some(game) = game else {
        return;
    };
    let id = game.player.class as u8;
    if active.id != id {
        if let Some(def) = class::def(id) {
            *active = ActiveClass {
                id,
                power: def.power,
            };
        }
    }
}
```

`Refusal`: replace `NotEnoughRage(u32)` with `NotEnough(Power)` and its message arm with `Refusal::NotEnough(p) => format!("Not enough {}.", p.word()),`; change `NoTarget`'s message to `"You have no target."`.

`activate` (replace the rage lines):

```rust
pub fn activate(
    rage: &mut Rage,
    mana: &mut Mana,
    buffs: &mut Buffs,
    cooldowns: &mut Cooldowns,
    ability: &Ability,
    aim: Option<Result<Entity, Refusal>>,
) -> Result<Cast, Refusal> {
    if !ability.usable {
        return Err(Refusal::NotYet);
    }
    let wait = cooldowns.left(ability.spell);
    if wait > 0.0 {
        return Err(Refusal::NotReady(wait));
    }
    let have = match ability.power {
        Power::Rage => rage.cur,
        Power::Mana => mana.cur,
    };
    if have < ability.cost as f32 {
        return Err(Refusal::NotEnough(ability.power));
    }
    let cast = match ability.effect {
        Effect::BattleShout => Cast::Shout,
        Effect::Charge => Cast::Charge(aim.unwrap_or(Err(Refusal::NoTarget))?),
        Effect::ThunderClap => Cast::Clap,
        Effect::Fireball | Effect::Frostbolt | Effect::FireBlast | Effect::FrostArmor => {
            Cast::Spell(ability.spell)
        }
    };
    match ability.power {
        Power::Rage => rage.cur -= ability.cost as f32,
        Power::Mana => mana.spend(ability.cost as f32),
    }
    if cast == Cast::Shout {
        buffs.shout = SHOUT_SECS;
    }
    let wait = moves::cooldown(ability.effect);
    if wait > 0.0 {
        cooldowns.start(ability.spell, wait);
    }
    Ok(cast)
}
```

`upkeep` gets `active: Res<ActiveClass>` and `mut mana: ResMut<Mana>`; gate the two rage gains with `if builds_rage(active.power)`, add `mana.tick(dt);` beside `rage.tick(dt)`, and on `Respawned` add `mana.refill();`. `input` gets `mut mana: ResMut<Mana>` (add to the five-resource tuple, which becomes a six-tuple) and passes `&mut mana` to `activate`. `plugin`:

```rust
    let start_mana = std::env::var("WOWSOULS_MANA")
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .unwrap_or(0.0)
        .max(0.0);
    ...
    app.insert_resource(Rage { cur: start, ..default() })
        .insert_resource(Mana { cur: start_mana, ..default() })
        .init_resource::<ActiveClass>()
        ...
        .add_systems(Update, (follow_class, refit, input, upkeep, moves::thunder_clap, moves::cast_play).chain().run_if(in_state(Phase::Playing)))
```
Document `WOWSOULS_MANA=<n>` in the module doc's Switches line.

- [ ] **Step 9: The pool follows level and gear**

In `crates/wow-souls/src/level.rs` `follow_level` add `mut mana: ResMut<crate::abilities::Mana>,` to the parameters and, right after `let character = match ... ;`:

```rust
    // The mana pool follows the class tables, Intellect and Spirit every frame (cheap, and a piece of
    // gear changes it without changing the level).
    if let Some(c) = &character {
        mana.sync(c.max_mana, c.total.spirit as f32);
    }
```
Add a pure test in `level.rs`: none needed beyond `Mana::sync` (covered in Step 7).

- [ ] **Step 10: The bar shows the class's power**

In `crates/wow-souls/src/abilities/ui.rs`:

```rust
use wow_world::class::Power;
use super::{AbilityArt, ActiveClass, Cooldowns, Hotbar, Mana, Rage};

const FILL_RAGE: Color = Color::srgb(0.78, 0.2, 0.12);
const FILL_MANA: Color = Color::srgb(0.2, 0.42, 0.85);

/// What the bar shows for `power`: the current value and the most there is.
pub fn bar_of(power: Power, rage: &Rage, mana: &Mana) -> (f32, f32) {
    match power {
        Power::Rage => (rage.cur, abilities::RAGE_MAX as f32),
        Power::Mana => (mana.cur, mana.max.max(1.0)),
    }
}
```
Replace the `FILL` constant by the two above (the spawn uses `FILL_RAGE`; `update_bar` recolours). `update_bar`:

```rust
fn update_bar(
    art: Option<Res<HudArt>>,
    active: Res<ActiveClass>,
    (rage, mana): (Res<Rage>, Res<Mana>),
    avatar: Query<&Stamina, With<Avatar>>,
    mut backing: Query<&mut Node, (With<Backing>, Without<Fill>)>,
    mut fill: Query<(&mut Node, &mut BackgroundColor), (With<Fill>, Without<Backing>)>,
) {
    let (Some(art), Ok(stamina)) = (art, avatar.single()) else {
        return;
    };
    let len = crate::hud::bar_length(stamina.max, plate(&art).w());
    if let Ok(mut n) = backing.single_mut() {
        if n.width != px(len) {
            n.width = px(len);
        }
    }
    if let Ok((mut n, mut color)) = fill.single_mut() {
        let (cur, max) = bar_of(active.power, &rage, &mana);
        let want = percent((cur / max * 100.0).clamp(0.0, 100.0));
        if n.width != want {
            n.width = want;
        }
        let tint = match active.power {
            Power::Rage => FILL_RAGE,
            Power::Mana => FILL_MANA,
        };
        if color.0 != tint {
            color.0 = tint;
        }
    }
}
```
`update_slots` takes `active`, `mana` and dims by `bar_of(active.power, ..).0 >= cost`. Test in `ui.rs` (new `#[cfg(test)] mod tests`):

```rust
    #[test]
    fn the_bar_shows_the_class_power() {
        let rage = Rage { cur: 40.0, ..Rage::default() };
        let mut mana = Mana::default();
        mana.sync(165.0, 22.0);
        mana.cur = 80.0;
        assert_eq!(bar_of(Power::Rage, &rage, &mana), (40.0, 100.0));
        assert_eq!(bar_of(Power::Mana, &rage, &mana), (80.0, 165.0));
        assert_eq!(bar_of(Power::Mana, &rage, &Mana::default()).1, 1.0, "never divides by zero");
    }
```

- [ ] **Step 11: Gate, then see it live**

Run the Task 1 Step 8 gate. Then a live check (unattended, scratch saves):

```bash
cd /c/Spel/wow-souls && S=$(mktemp -d) && printf 'wschar 2\nname Merlin\nrace human\nclass mage\nsex male\nskin 0\nface 0\nhair_style 0\nhair_color 0\nfacial_hair 0\n' > "$S/character-v1.txt" && cargo build -j 4 -p wow-souls && WOWSOULS_CHARACTER="$S/character-v1.txt" WOWSOULS_CLOCK_SAVE="$S/clock.txt" WOWSOULS_ITEMS_SAVE="$S/items.txt" WOWSOULS_QUESTS_SAVE="$S/quests.txt" WOWSOULS_SOULS_SAVE="$S/souls.txt" WOWSOULS_AT=-8906,-136 WOWSOULS_CHECK=8 WOWSOULS_SHOT="$S/mage.png" WOWSOULS_SHOT_AT=5 timeout 90 ./target/debug/wow-souls.exe 2>&1 | grep -i "level:\|equip:\|abilities\|gear:" | head -20; ls "$S"
```
Expected log lines: `equip: starting kit on:` with the five mage pieces (MainHand 35 ...), `level: 1 -> health 52`, no panic; the screenshot `mage.png` shows a blue thin bar under the stamina bar and the mage in the Apprentice's robe with a staff. Look at the picture (Read the PNG) and describe what you see in the log.

- [ ] **Step 12: Log and HANDOVER**

Write `docs/log/2026-10-14-mage-s2.md` and set HANDOVER item 1 to "S2 done: a Mage can be made, 165 mana / 52 HP, mana bar". No commit.

---

### Task 3: The cast clips: extractor family, survey, loader (S3)

**Files:**
- Create: `crates/dsr-cache/src/casts.rs`, `crates/wow-souls/src/spells/mod.rs`, `crates/wow-souls/src/spells/cast.rs`
- Modify: `crates/dsr-cache/src/params.rs`, `crates/dsr-cache/src/cache.rs` (visibility, `extract` tail), `crates/dsr-cache/src/main.rs`, `crates/wow-souls/src/main.rs` (`mod spells;`), `crates/wow-souls/src/boot.rs:98-102` (`.add_plugins(crate::spells::plugin)`), `crates/wow-souls/src/dsclip.rs`, `crates/wow-souls/src/dsview.rs`
- Test: new modules' tests; install tests

**Interfaces:**
- Consumes: `dsr::Player::flags(stem) -> Vec<wscl::FlagWindow>`, `cache::extract_clip(&Player, &WowSkel, &Rig, &str) -> Result<Extracted>` (made `pub(crate)`), `dsclip::{read_clip, checked_manifest, cache_dir}`.
- Produces: `cache/dsr/casts.txt` (format below); `spells::cast::{Role, Timing, Bullet, Clip, Kit}`; `ClipId::Cast(Role)`; `clips_of(.., casts: Option<&cast::Kit>, id)`.

`casts.txt` format:

```
casts-version 1
source <size> <mtime> <path>          (one per animation archive, as interaction.txt)
cast <role> <clip> <duration> <release> <recover>
bullet <role> <life> <dist> <speed0> <speed1> <accel> <gravity> <homing_deg> <radius> <burst>
```

- [ ] **Step 1: Failing tests for the release rule**

Create `crates/dsr-cache/src/casts.rs` with only the tests first:

```rust
//! The Mage's casts ...

#[cfg(test)]
mod tests {
    use super::*;

    fn flag(flag: i32, start: f32, end: f32) -> wscl::FlagWindow {
        wscl::FlagWindow { flag, start, end }
    }

    #[test]
    fn the_release_is_where_flag_90_opens_and_the_recovery_where_89_does() {
        // a00_6205: wind-up 0.23-0.77, release 0.77-1.23, recovery 1.23-1.73.
        let f = [flag(91, 0.23, 0.77), flag(90, 0.77, 1.23), flag(89, 1.23, 1.73)];
        assert_eq!(timing(&f, 1.73), Some((0.77, 1.23)));
    }

    #[test]
    fn a_clip_with_no_recovery_window_recovers_when_it_ends() {
        // a00_6217: wind-up 0.20-0.70, release 0.70-1.47, no 89.
        let f = [flag(91, 0.20, 0.70), flag(90, 0.70, 1.47)];
        assert_eq!(timing(&f, 1.47), Some((0.70, 1.47)));
    }

    #[test]
    fn a_clip_with_no_release_window_is_no_cast() {
        // a00_6200 has only flag 91.
        let f = [flag(91, 0.17, 1.90)];
        assert_eq!(timing(&f, 1.90), None);
        assert_eq!(timing(&[], 1.0), None);
    }

    #[test]
    fn a_release_after_the_recovery_or_past_the_end_is_refused() {
        let f = [flag(90, 1.0, 1.5), flag(89, 0.5, 1.5)];
        assert_eq!(timing(&f, 2.0), None, "recovery before the release");
        let f = [flag(90, 3.0, 4.0)];
        assert_eq!(timing(&f, 2.0), None, "release after the clip ends");
    }

    #[test]
    fn the_roles_name_four_distinct_clips_and_two_missiles() {
        let names: std::collections::BTreeSet<_> = ROLES.iter().map(|r| r.clip).collect();
        assert_eq!(names.len(), 4);
        assert_eq!(ROLES.iter().filter(|r| r.magic.is_some()).count(), 2);
        assert_eq!(
            ROLES.iter().map(|r| r.name).collect::<Vec<_>>(),
            vec!["fireball", "frostbolt", "fire_blast", "frost_armor"]
        );
    }
}
```

Run: `cargo test -j 4 -p dsr-cache casts 2>&1 | tail` → FAIL (module not declared / `timing` missing).

- [ ] **Step 2: `params.rs`: the `Magic` and `Bullet` rows**

In `crates/dsr-cache/src/params.rs` add after `Think`:

```rust
/// A `Magic` row: what a spell flies (`refCategory` 1: `refId` is a `Bullet` row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MagicRow {
    pub ref_id: i64,
    pub ref_category: i64,
    pub ref_type: i64,
}

/// A `Bullet` row: a missile's flight in Dark Souls metres, seconds and degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BulletRow {
    pub life: f32,
    pub dist: f32,
    pub init_speed: f32,
    pub max_speed: f32,
    pub accel_time: f32,
    pub gravity: f32,
    pub homing_deg: i64,
    pub radius: f32,
    pub hit_bullet: i64,
}
```
`Params` gets `magic: Param, bullet: Param`, `load` gets `magic: take("Magic")?, bullet: take("Bullet")?`, and:

```rust
    /// The `Magic` row `id` (3000 is the first sorcery, 4000 the first pyromancy).
    pub fn magic(&self, id: i32) -> Result<MagicRow> {
        let p = &self.magic;
        let r = p.row_of(id).with_context(|| format!("no Magic row {id}"))?;
        Ok(MagicRow {
            ref_id: p.int(r, "refId")?,
            ref_category: p.int(r, "refCategory")?,
            ref_type: p.int(r, "refType")?,
        })
    }

    /// The `Bullet` row `id`.
    pub fn bullet(&self, id: i32) -> Result<BulletRow> {
        let p = &self.bullet;
        let r = p.row_of(id).with_context(|| format!("no Bullet row {id}"))?;
        Ok(BulletRow {
            life: p.float(r, "life")?,
            dist: p.float(r, "dist")?,
            init_speed: p.float(r, "initVellocity")?,
            max_speed: p.float(r, "maxVellocity")?,
            accel_time: p.float(r, "accelTime")?,
            gravity: p.float(r, "gravityInRange")?,
            homing_deg: p.int(r, "homingAngle")?,
            radius: p.float(r, "hitRadius")?,
            hit_bullet: p.int(r, "HitBulletID")?,
        })
    }
```
(`homingAngle` is an s16: read with `int`; the others are f32; `HitBulletID` s32. The spellings `Vellocity` are the install's.) Add an install test in this file's tests:

```rust
    #[test]
    fn soul_arrow_and_fireball_fly_as_the_survey_read_them() {
        let Some(dsr) = std::env::var_os("DSR_DIR") else { return };
        let p = load(std::path::Path::new(&dsr)).unwrap();
        let soul = p.bullet(p.magic(3000).unwrap().ref_id as i32).unwrap();
        assert_eq!((soul.life, soul.max_speed, soul.homing_deg), (1.1, 30.0, 40));
        assert!((soul.radius - 0.1).abs() < 1e-6);
        let fire = p.bullet(p.magic(4000).unwrap().ref_id as i32).unwrap();
        assert_eq!((fire.max_speed, fire.gravity, fire.hit_bullet), (15.0, 9.8, 6010));
        assert_eq!(p.magic(3000).unwrap().ref_category, 1, "refId is a Bullet row");
    }
```
(If the file has no `#[cfg(test)] mod tests`, create one with `use super::*;`.)

- [ ] **Step 3: `casts.rs`: the release rule, the roles, `extract` and `survey`**

Replace the file's head (above the tests):

```rust
//! The Mage's casts: Dark Souls' own cast animations and the numbers of the missile each spell flies,
//! published as `casts.txt` beside the clips (spec `docs/superpowers/specs/2026-10-14-mage-class-design.md`,
//! section 3). A separate publication like `interaction.txt`: optional clips, independent of the combat
//! kits.
//!
//! A cast clip carries three `ChrActionFlag` windows (hypothesis, checked in motion): 91 the wind-up, 90
//! the release, 89 the recovery. The release instant is where flag 90 opens; the body may act again
//! where flag 89 opens (the clip's end when it has none).

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{ensure, Context, Result};

use crate::cache::{extract_clip, source_line, summary, write_pair};
use crate::{bonemap, dsr, params, wow};

pub const FLAG_WINDUP: i32 = 91;
pub const FLAG_RELEASE: i32 = 90;
pub const FLAG_RECOVER: i32 = 89;

/// One of the Mage's casts: the DS clip it wears and the `Magic` row whose `Bullet` it borrows its homing
/// and hit radius from (`None`: no missile).
pub struct Role {
    pub name: &'static str,
    pub clip: &'static str,
    pub magic: Option<i32>,
}

/// The first picks, from the TAE survey of `a00_6200-6226`; `casts-survey` and the frame series confirm
/// or swap them.
pub const ROLES: [Role; 4] = [
    Role {
        name: "fireball",
        clip: "a00_6205",
        magic: Some(4000),
    },
    Role {
        name: "frostbolt",
        clip: "a00_6209",
        magic: Some(3000),
    },
    Role {
        name: "fire_blast",
        clip: "a00_6207",
        magic: None,
    },
    Role {
        name: "frost_armor",
        clip: "a00_6217",
        magic: None,
    },
];

/// `(release, recover)` of a cast clip from its flag windows; `None` when it has no release window or
/// the windows do not make a cast (the release must come before the recovery, inside the clip).
pub fn timing(flags: &[wscl::FlagWindow], duration: f32) -> Option<(f32, f32)> {
    let open = |flag: i32| {
        flags
            .iter()
            .filter(|f| f.flag == flag)
            .map(|f| f.start)
            .reduce(f32::min)
    };
    let release = open(FLAG_RELEASE)?;
    let recover = open(FLAG_RECOVER).unwrap_or(duration);
    (release >= 0.0 && release < recover && release < duration && recover <= duration + 1.0e-3)
        .then_some((release, recover.min(duration)))
}

/// Retargets the roles' clips through the same gates as every other family and writes `casts.txt`.
pub fn extract(dsr_dir: &Path, wow_dir: &Path, out: &Path) -> Result<()> {
    let player = dsr::load(dsr_dir)?;
    let skel = wow::human_male(wow_dir)?;
    let params = params::load(dsr_dir)?;
    let mut lines = String::from("casts-version 1\n");
    for archive in dsr::animation_archives(dsr_dir)? {
        writeln!(lines, "{}", source_line(&archive)?)?;
    }
    for role in &ROLES {
        let e = extract_clip(&player, &skel, &bonemap::HUMAN_MALE_RIG, role.clip)
            .with_context(|| format!("cast {} ({})", role.name, role.clip))?;
        ensure!(
            e.r.wow.windows(wscl::FLAG_INVINCIBLE).next().is_none() && e.r.wow.hits.is_empty(),
            "{}: a cast carries no hit and no invulnerability",
            role.clip
        );
        let duration = e.r.wow.duration();
        let (release, recover) = timing(&player.flags(role.clip), duration).with_context(|| {
            format!(
                "{} has no release window (flag {FLAG_RELEASE}): pick another clip for {}",
                role.clip, role.name
            )
        })?;
        println!(
            "cast {}: {}  release {release:.3} recover {recover:.3} of {duration:.3}",
            role.name,
            summary(role.clip, &e.r)
        );
        write_pair(out, "human_male", "c0000", role.clip, &e.r)?;
        writeln!(
            lines,
            "cast {} {} {duration:.3} {release:.3} {recover:.3}",
            role.name, role.clip
        )?;
        if let Some(magic) = role.magic {
            let m = params.magic(magic)?;
            ensure!(m.ref_category == 1, "Magic {magic}: refId is not a Bullet row");
            let b = params.bullet(m.ref_id as i32)?;
            writeln!(
                lines,
                "bullet {} {} {} {} {} {} {} {} {} {}",
                role.name,
                b.life,
                b.dist,
                b.init_speed,
                b.max_speed,
                b.accel_time,
                b.gravity,
                b.homing_deg,
                b.radius,
                b.hit_bullet
            )?;
        }
    }
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("casts.txt"), lines)?;
    Ok(())
}

/// `casts-survey`: every clip of the cast blocks `a00_6200-6226`, `6300-6326`, `6400-6426`, `6500-6526`
/// with its flag windows, so a role's clip can be chosen by the rule. Read-only: nothing is written.
pub fn survey(dsr_dir: &Path, wow_dir: &Path) -> Result<()> {
    let player = dsr::load(dsr_dir)?;
    let skel = wow::human_male(wow_dir)?;
    for base in [6200, 6300, 6400, 6500] {
        for id in base..=base + 26 {
            let name = format!("a00_{id}");
            let flags = player.flags(&name);
            if player.animation(&name).is_none() {
                continue;
            }
            let windows = |flag: i32| {
                flags
                    .iter()
                    .filter(|f| f.flag == flag)
                    .map(|f| format!("{:.2}-{:.2}", f.start, f.end))
                    .collect::<Vec<_>>()
                    .join("+")
            };
            let gates = match extract_clip(&player, &skel, &bonemap::HUMAN_MALE_RIG, &name) {
                Ok(e) => {
                    let d = e.r.wow.duration();
                    match timing(&flags, d) {
                        Some((r, c)) => format!("OK len {d:.2} release {r:.2} recover {c:.2}"),
                        None => format!("no release window, len {d:.2}"),
                    }
                }
                Err(why) => format!("gate: {why}"),
            };
            println!(
                "{name} 91:[{}] 90:[{}] 89:[{}]  {gates}",
                windows(FLAG_WINDUP),
                windows(FLAG_RELEASE),
                windows(FLAG_RECOVER)
            );
        }
    }
    Ok(())
}
```
`player.animation(name)` is the lookup `fn animation(&self, name)` seen in `dsr.rs` (the TAE record by stem; check its visibility `pub`; if it is private, drop the `continue` guard: `player.flags` of a missing clip is empty and `extract_clip` fails cleanly and prints `gate:`).

- [ ] **Step 4: Wire the helpers, the commands and the `extract` tail**

In `crates/dsr-cache/src/cache.rs` make these `pub(crate)`: `fn write_pair`, `fn extract_clip`, `fn summary` and `struct Extracted` (+ its fields `r` and `basis` if the compiler asks). `source_line` is already `pub(crate)`. At the end of `extract` change the last two lines to:

```rust
    extract_locomotion(dsr_dir, wow_dir, out)?;
    crate::casts::extract(dsr_dir, wow_dir, out)
```
In `crates/dsr-cache/src/main.rs`: add `mod casts;` (alphabetical after `mod cache;`), the doc lines `wow-souls-dsr casts [--dsr <dir>] [--wow <Data dir>] [--out <dir>]` and `wow-souls-dsr casts-survey [--dsr <dir>] [--wow <Data dir>]`, and in the dispatch:

```rust
        "casts" => casts::extract(&a.dsr, &a.wow, &a.out),
        "casts-survey" => casts::survey(&a.dsr, &a.wow),
```
and add `casts|casts-survey` to the usage string.

- [ ] **Step 5: Run the extractor tests, then the survey on the real install**

Run: `cd /c/Spel/wow-souls && DSR_DIR="C:/Spel/Dark Souls Remastered" WOW_DATA="C:/Spel/World of Warcraft 1.12/Data" cargo test -j 4 -p dsr-cache 2>&1 | tail -15`
Expected: PASS (the `casts` unit tests, the new params install test).

Then the survey (read-only):

```bash
cd /c/Spel/wow-souls && cargo build -j 4 -p dsr-cache && ./target/debug/wow-souls-dsr.exe casts-survey 2>&1 | tee /tmp/casts-survey.txt | head -120
```
Expected: one line per clip, for example `a00_6205 91:[0.23-0.77] 90:[0.77-1.23] 89:[1.23-1.73]  OK len 1.73 release 0.77 recover 1.23`. The four picks must read `OK`. If one reads `no release window` or `gate: ...`, choose another `OK` clip for that role by the rule "release between 0.1 and 1.0 s, length 0.9 to 2.1 s, closest to the DS spell's own type" and edit `ROLES` and the `roles_name_four_distinct_clips` test. Copy the table into `docs/log/2026-10-14-mage-s3.md`.

- [ ] **Step 6: Write the cache**

Run: `./target/debug/wow-souls-dsr.exe casts 2>&1 | tail -12 && cat cache/dsr/casts.txt`
Expected: `cast fireball a00_6205 1.733 0.767 1.233` style lines (numbers from the clips), two `bullet` lines (fireball, frostbolt), and the `.wscl` pairs under `cache/dsr/human_male/` and `cache/dsr/c0000/`. Also run the full `extract`? No: `casts` alone is enough; `extract` will include it next time.

- [ ] **Step 7: Failing tests for the game-side kit**

Create `crates/wow-souls/src/spells/cast.rs` with these tests first (bottom of the file), then the code in Step 8:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "casts-version 1\n\
        source 100 5 C:/x/c0000_a00_md.anibnd.dcx\n\
        cast fireball a00_6205 1.733 0.767 1.233\n\
        bullet fireball 0.5 10 15 15 0 9.8 10 0.25 6010\n\
        cast frostbolt a00_6209 1.9 0.7 1.333\n\
        bullet frostbolt 1.1 20 4 30 0.32 0 40 0.1 -1\n\
        cast fire_blast a00_6207 0.967 0.133 0.467\n\
        cast frost_armor a00_6217 1.467 0.7 1.467\n";

    #[test]
    fn the_manifest_gives_each_role_its_clip_timing_and_bullet() {
        let p = parse(TEXT).unwrap();
        assert_eq!(p.len(), 4);
        let fireball = p.iter().find(|c| c.role == Role::Fireball).unwrap();
        assert_eq!(fireball.clip, "a00_6205");
        assert_eq!(
            fireball.timing,
            Timing {
                duration: 1.733,
                release: 0.767,
                recover: 1.233
            }
        );
        assert_eq!(fireball.bullet.unwrap().burst, 6010);
        let frost = p.iter().find(|c| c.role == Role::Frostbolt).unwrap();
        assert_eq!(frost.bullet.unwrap().homing_deg, 40.0);
        let blast = p.iter().find(|c| c.role == Role::FireBlast).unwrap();
        assert!(blast.bullet.is_none(), "an instant spell flies nothing");
    }

    #[test]
    fn a_wrong_version_a_missing_role_or_a_bad_timing_is_an_error() {
        assert!(parse("casts-version 2\n").is_err());
        let missing = TEXT.replace("cast frost_armor a00_6217 1.467 0.7 1.467\n", "");
        assert!(parse(&missing).unwrap_err().contains("frost_armor"));
        let bad = TEXT.replace("0.767 1.233", "1.5 1.2");
        assert!(parse(&bad).unwrap_err().contains("fireball"), "release after recovery");
        let unknown = format!("{TEXT}cast lightning a00_6200 1 0.5 0.9\n");
        assert!(parse(&unknown).unwrap_err().contains("lightning"));
    }

    #[test]
    fn a_spell_id_names_its_role() {
        assert_eq!(Role::of_spell(133), Some(Role::Fireball));
        assert_eq!(Role::of_spell(116), Some(Role::Frostbolt));
        assert_eq!(Role::of_spell(2136), Some(Role::FireBlast));
        assert_eq!(Role::of_spell(168), Some(Role::FrostArmor));
        assert_eq!(Role::of_spell(100), None, "Charge is no cast");
        for r in Role::ALL {
            assert_eq!(Role::of_spell(r.spell()), Some(r));
        }
    }
}
```

- [ ] **Step 8: `spells/cast.rs`**

```rust
//! The Mage's cast clips: Dark Souls' own cast animations from `cache/dsr/casts.txt` (written by
//! `wow-souls-dsr casts`, spec `docs/superpowers/specs/2026-10-14-mage-class-design.md`, section 3).
//! Each spell wears one clip; its flag windows give the action machine its release and recovery.

use std::path::Path;

use bevy::prelude::*;

/// The Mage's four casts, by the spell each is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Fireball,
    Frostbolt,
    FireBlast,
    FrostArmor,
}

impl Role {
    pub const ALL: [Role; 4] = [
        Role::Fireball,
        Role::Frostbolt,
        Role::FireBlast,
        Role::FrostArmor,
    ];

    /// The name `casts.txt` gives it.
    pub fn name(self) -> &'static str {
        match self {
            Role::Fireball => "fireball",
            Role::Frostbolt => "frostbolt",
            Role::FireBlast => "fire_blast",
            Role::FrostArmor => "frost_armor",
        }
    }

    /// The spell id (rank 1).
    pub fn spell(self) -> u32 {
        match self {
            Role::Fireball => 133,
            Role::Frostbolt => 116,
            Role::FireBlast => 2136,
            Role::FrostArmor => 168,
        }
    }

    pub fn of_spell(spell: u32) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.spell() == spell)
    }

    fn from_name(name: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.name() == name)
    }
}

/// A cast's timing in seconds from its start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timing {
    pub duration: f32,
    pub release: f32,
    pub recover: f32,
}

/// The Dark Souls `Bullet` row a spell borrows (metres, seconds, degrees).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bullet {
    pub life: f32,
    pub dist: f32,
    pub speed0: f32,
    pub speed1: f32,
    pub accel: f32,
    pub gravity: f32,
    pub homing_deg: f32,
    pub radius: f32,
    pub burst: i64,
}

/// One role's lines of `casts.txt`.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub role: Role,
    pub clip: String,
    pub timing: Timing,
    pub bullet: Option<Bullet>,
}

/// Reads `casts.txt`; every role must be there, with a release before its recovery inside the clip.
pub fn parse(text: &str) -> Result<Vec<Line>, String> {
    if text.lines().next() != Some("casts-version 1") {
        return Err("wrong casts cache version".into());
    }
    let num = |w: &str, line: &str| -> Result<f32, String> {
        w.parse::<f32>()
            .ok()
            .filter(|n| n.is_finite())
            .ok_or_else(|| format!("bad number in `{line}`"))
    };
    let mut lines: Vec<Line> = Vec::new();
    for line in text.lines().skip(1) {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.as_slice() {
            ["cast", role, clip, d, r, c] => {
                let role = Role::from_name(role).ok_or_else(|| format!("unknown role `{role}`"))?;
                let timing = Timing {
                    duration: num(d, line)?,
                    release: num(r, line)?,
                    recover: num(c, line)?,
                };
                if !(timing.release >= 0.0
                    && timing.release < timing.recover
                    && timing.recover <= timing.duration + 1.0e-3)
                {
                    return Err(format!("{}: release must come before the recovery", role.name()));
                }
                lines.push(Line {
                    role,
                    clip: (*clip).to_string(),
                    timing,
                    bullet: None,
                });
            }
            ["bullet", role, life, dist, s0, s1, accel, gravity, homing, radius, burst] => {
                let role = Role::from_name(role).ok_or_else(|| format!("unknown role `{role}`"))?;
                let b = Bullet {
                    life: num(life, line)?,
                    dist: num(dist, line)?,
                    speed0: num(s0, line)?,
                    speed1: num(s1, line)?,
                    accel: num(accel, line)?,
                    gravity: num(gravity, line)?,
                    homing_deg: num(homing, line)?,
                    radius: num(radius, line)?,
                    burst: burst
                        .parse::<i64>()
                        .map_err(|_| format!("bad burst in `{line}`"))?,
                };
                lines
                    .iter_mut()
                    .find(|l| l.role == role)
                    .ok_or_else(|| format!("bullet before cast: {}", role.name()))?
                    .bullet = Some(b);
            }
            _ => {}
        }
    }
    for role in Role::ALL {
        if !lines.iter().any(|l| l.role == role) {
            return Err(format!("incomplete casts cache: no {}", role.name()));
        }
    }
    Ok(lines)
}

/// A cast clip on both rigs with its timing.
pub struct Clip {
    pub role: Role,
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
    pub timing: Timing,
    pub bullet: Option<Bullet>,
}

/// The loaded casts (absent without a fresh Dark Souls cache: the Mage's spells then stay unusable).
#[derive(Resource)]
pub struct Kit {
    pub clips: Vec<Clip>,
}

impl Kit {
    pub fn clip(&self, role: Role) -> Option<&Clip> {
        self.clips.iter().find(|c| c.role == role)
    }

    /// The timing of the cast of `spell`.
    pub fn timing(&self, spell: u32) -> Option<Timing> {
        self.clip(Role::of_spell(spell)?).map(|c| c.timing)
    }

    pub fn load(dir: &Path) -> Result<Kit, String> {
        let text = std::fs::read_to_string(dir.join("casts.txt")).map_err(|e| e.to_string())?;
        let parsed = parse(&text)?;
        // The same strict source checks as the other publications; the common cache is required by the
        // avatar's pose pipeline anyway.
        crate::dsclip::checked_manifest(dir)?;
        let mut clips = Vec::new();
        for l in parsed {
            let wow = crate::dsclip::read_clip(dir, "human_male", &l.clip)?;
            let ds = crate::dsclip::read_clip(dir, "c0000", &l.clip)?;
            if (wow.duration() - l.timing.duration).abs() > 0.02 {
                return Err(format!(
                    "{}: the clip is {:.3} s, the manifest says {:.3}",
                    l.role.name(),
                    wow.duration(),
                    l.timing.duration
                ));
            }
            if wow.windows(wscl::FLAG_INVINCIBLE).next().is_some() {
                return Err(format!("{}: a cast confers no invulnerability", l.role.name()));
            }
            info!(
                "spells: cast {} {} ({:.3} s, release {:.3}, recover {:.3})",
                l.role.name(),
                l.clip,
                l.timing.duration,
                l.timing.release,
                l.timing.recover
            );
            clips.push(Clip {
                role: l.role,
                wow,
                ds,
                timing: l.timing,
                bullet: l.bullet,
            });
        }
        Ok(Kit { clips })
    }
}

/// Loads the casts at startup; without them the Mage's spells say "You cannot do that yet."
pub fn load_kit(mut commands: Commands) {
    match Kit::load(&crate::dsclip::cache_dir()) {
        Ok(k) => {
            commands.insert_resource(k);
        }
        Err(why) => {
            warn!("spells: no Dark Souls cast clips ({why}); the mage's spells stay unusable");
        }
    }
}
```
Create `crates/wow-souls/src/spells/mod.rs`:

```rust
//! The Mage's spells (spec `docs/superpowers/specs/2026-10-14-mage-class-design.md`): the cast clips
//! ([`cast`]), and in later tasks the cast's start, its release, the missile's hits and the buffs.

pub mod cast;

use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, cast::load_kit);
}
```
Add `mod spells;` to `crates/wow-souls/src/main.rs` (after `mod species;`) and `.add_plugins(crate::spells::plugin)` after `.add_plugins(crate::abilities::plugin)` in `boot.rs`.

- [ ] **Step 9: `ClipId::Cast` in the pose pipeline**

In `crates/wow-souls/src/dsclip.rs`:

1. `ClipId` gains `Cast(crate::spells::cast::Role),` (doc: "A cast clip of the Mage ([`crate::spells`])").
2. `clips_of` gains a parameter before `id`: `casts: Option<&'a crate::spells::cast::Kit>,` and an arm:

```rust
        ClipId::Cast(role) => {
            let c = casts?.clip(role)?;
            Some((&c.wow, &c.ds))
        }
```
3. `pose` gains `casts: Option<Res<crate::spells::cast::Kit>>,` (next to `interaction`) and passes `casts.as_deref()` in its `clips_of(...)` call (between `interaction.as_deref()` and `clip`).
4. Fix every other `clips_of(` caller the compiler names (`dsview.rs`), passing `None` where no kit is at hand.

In `crates/wow-souls/src/dsview.rs` `resolve` gets `casts: Option<&crate::spells::cast::Kit>` and, before the gait arm:

```rust
        .or_else(|| {
            casts
                .and_then(|k| {
                    k.clips
                        .iter()
                        .find(|c| c.role.name() == want || c.wow.name == want)
                })
                .map(|c| ClipId::Cast(c.role))
        })
```
and its caller passes the kit resource (`Option<Res<crate::spells::cast::Kit>>` in the system's parameters, `.as_deref()`).

- [ ] **Step 10: Install test and the clips in motion**

In `spells/cast.rs` tests add:

```rust
    #[test]
    fn the_installed_cache_loads_when_it_has_casts() {
        let Some(_) = std::env::var_os("DSR_DIR") else { return };
        let dir = crate::dsclip::cache_dir();
        if !dir.join("casts.txt").exists() {
            return;
        }
        let kit = Kit::load(&dir).unwrap();
        for r in Role::ALL {
            let c = kit.clip(r).unwrap_or_else(|| panic!("{}", r.name()));
            assert!(c.timing.release > 0.0 || r == Role::FireBlast, "{}", r.name());
            assert!(c.wow.duration() >= c.timing.recover - 0.02);
        }
        assert!(kit.clip(Role::Fireball).unwrap().bullet.is_some());
        assert!(kit.clip(Role::FrostArmor).unwrap().bullet.is_none());
    }
```

Run the gate (Task 1 Step 8). Then look at each clip in motion with the side-by-side (`WOWSOULS_DSVIEW=<role>`; see `dsview.rs` header for `@<s>` holds and `WOWSOULS_OBSERVE`/`WOWSOULS_SHOT`):

```bash
cd /c/Spel/wow-souls && for r in fireball frostbolt fire_blast frost_armor; do WOWSOULS_DSVIEW=$r WOWSOULS_OBSERVE=1 WOWSOULS_SHOT="/tmp/cast-$r.png" WOWSOULS_SHOT_AT=3,3.3,3.6,3.9,4.2,4.5 WOWSOULS_CHECK=6 timeout 90 ./target/debug/wow-souls.exe 2>&1 | grep -i "dsview\|dsclip" | head -3; done; ls /tmp/cast-*.png
```
(Use the scratch-save environment of Task 2 Step 11 and a character file with `class mage`. If `WOWSOULS_SHOT_AT` accepts a list the files are numbered; otherwise run once per time.) Look at the frames with Read: a cast that tears, floats, or keeps the staff hand empty-and-wrong is swapped in `ROLES` (Step 5 rule) and the cache regenerated. Record each verdict in the log.

- [ ] **Step 11: Log and HANDOVER**

`docs/log/2026-10-14-mage-s3.md`: the survey table, the picks, the frame verdicts. HANDOVER item 1: "S3 done: casts.txt (4 DS clips, release / recover from flags), `ClipId::Cast`, `WOWSOULS_DSVIEW=<role>`". No commit.

---

### Task 4: `ActionState::Cast` and Fireball end to end (S4)

**Files:**
- Create: `crates/wow-world/src/spells.rs`
- Modify: `crates/wow-world/src/lib.rs`, `crates/wow-souls/src/combat/action.rs`, `crates/wow-souls/src/combat/mod.rs`, `crates/wow-souls/src/dsclip.rs:778-793`, `crates/wow-souls/src/projectiles.rs`, `crates/wow-souls/src/spells/mod.rs`, `crates/wow-souls/src/abilities/mod.rs`, `crates/wow-souls/src/abilities/fx.rs`, `crates/wow-souls/src/abilities/moves.rs`, `docs/superpowers/specs/2026-10-14-mage-class-design.md`
- Test: those files' test modules

**Interfaces:**
- Consumes: `spells::cast::{Role, Timing, Kit}` (Task 3), `abilities::{Mana, activate, Cast::Spell, fx::{Fx, Kind, spawn, sound}}` (Tasks 1-2), `projectiles::Projectile`.
- Produces: `wow_world::spells::{SpellDef, Delivery, Dot, Slow, School, def(u32) -> Option<&'static SpellDef>, roll(&SpellDef, u: f32) -> f32}`; `combat::{CastSpec { duration, release, recover }, Cast { t, spell, spec, serial, target }, ActionState::Cast, Action::can_cast(), Action::begin_cast(spell, spec, target) -> bool}`; `spells::{CastPlan { spec, target }, plan(..) -> Result<CastPlan, Refusal>, aim_instant(..)}`; `projectiles::{SpellFlight, SpellHit, steer}`.

- [ ] **Step 1: The spells' WoW numbers (failing tests, then code)**

Create `crates/wow-world/src/spells.rs` (tests first):

```rust
//! What the Mage's four spells do, in WoW 1.12's rank 1 numbers (vMaNGOS `spell_template`, build 5875,
//! and the client's `SpellCastTimes` / `SpellDuration` / `SpellRange` rows read 2026-10-14). Pure data
//! and rules; the game's `spells` module delivers them. Spec
//! `docs/superpowers/specs/2026-10-14-mage-class-design.md`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_spells_are_rank_one_as_the_database_has_them() {
        let f = def(133).unwrap();
        assert_eq!(f.damage, (14, 22), "Fireball: basePoints 13, die 9");
        assert_eq!(f.delivery, Delivery::Missile { speed: 24.0 });
        assert_eq!((f.range, f.cooldown), (35.0, 0.0));
        let d = f.dot.unwrap();
        assert_eq!((d.per_tick, d.ticks, d.every), (1.0, 2, 2.0), "2 fire damage over 4 s");
        let b = def(116).unwrap();
        assert_eq!(b.damage, (18, 20), "Frostbolt: basePoints 17, die 3");
        assert_eq!(b.delivery, Delivery::Missile { speed: 28.0 });
        assert_eq!(b.range, 30.0);
        let s = b.slow.unwrap();
        assert_eq!((s.share, s.secs), (0.42, 5.0), "-41 base points, SpellDuration 28 = 5 s");
        let x = def(2136).unwrap();
        assert_eq!(x.damage, (24, 32), "Fire Blast: basePoints 23, die 9");
        assert_eq!((x.delivery, x.range, x.cooldown), (Delivery::Instant, 20.0, 8.0));
        let a = def(168).unwrap();
        assert_eq!((a.delivery, a.armor, a.secs), (Delivery::SelfBuff, 30, 1800.0));
        assert!(def(100).is_none(), "Charge is a warrior's");
    }

    #[test]
    fn the_damage_roll_covers_the_whole_range_and_nothing_else() {
        let f = def(133).unwrap();
        assert_eq!(roll(f, 0.0), 14.0);
        assert_eq!(roll(f, 0.999_999), 22.0);
        assert_eq!(roll(f, 0.5), 18.0);
        for i in 0..=100 {
            let r = roll(f, i as f32 / 100.0);
            assert!((14.0..=22.0).contains(&r) && r.fract() == 0.0, "{r}");
        }
        assert_eq!(roll(def(168).unwrap(), 0.7), 0.0, "a buff deals nothing");
    }
}
```
Run: `cargo test -j 4 -p wow-world spells 2>&1 | tail` → FAIL. Then add `pub mod spells;` to `lib.rs` (after `shop`) and write above the tests:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum School {
    Fire,
    Frost,
}

/// How a spell reaches its target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Delivery {
    /// A missile at this speed (yd/s), homing on the target.
    Missile { speed: f32 },
    /// A hit on a target in range, no flight.
    Instant,
    /// A buff on the caster.
    SelfBuff,
}

/// Damage over time after the hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dot {
    pub per_tick: f32,
    pub ticks: u32,
    pub every: f32,
}

/// A movement slow after the hit: `share` of the speed is taken for `secs`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slow {
    pub share: f32,
    pub secs: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpellDef {
    pub spell: u32,
    pub school: School,
    pub delivery: Delivery,
    /// The inclusive range one cast rolls (0, 0 for a buff).
    pub damage: (u32, u32),
    pub dot: Option<Dot>,
    pub slow: Option<Slow>,
    /// Armor a buff adds, and for how long (s).
    pub armor: u32,
    pub secs: f32,
    /// How far it reaches (yd).
    pub range: f32,
    /// Seconds before it can be cast again.
    pub cooldown: f32,
}

pub const SPELLS: [SpellDef; 4] = [
    SpellDef {
        spell: 133,
        school: School::Fire,
        delivery: Delivery::Missile { speed: 24.0 },
        damage: (14, 22),
        dot: Some(Dot {
            per_tick: 1.0,
            ticks: 2,
            every: 2.0,
        }),
        slow: None,
        armor: 0,
        secs: 0.0,
        range: 35.0,
        cooldown: 0.0,
    },
    SpellDef {
        spell: 116,
        school: School::Frost,
        delivery: Delivery::Missile { speed: 28.0 },
        damage: (18, 20),
        dot: None,
        slow: Some(Slow {
            share: 0.42,
            secs: 5.0,
        }),
        armor: 0,
        secs: 0.0,
        range: 30.0,
        cooldown: 0.0,
    },
    SpellDef {
        spell: 2136,
        school: School::Fire,
        delivery: Delivery::Instant,
        damage: (24, 32),
        dot: None,
        slow: None,
        armor: 0,
        secs: 0.0,
        range: 20.0,
        cooldown: 8.0,
    },
    SpellDef {
        spell: 168,
        school: School::Frost,
        delivery: Delivery::SelfBuff,
        damage: (0, 0),
        dot: None,
        slow: None,
        armor: 30,
        secs: 1800.0,
        range: 0.0,
        cooldown: 0.0,
    },
];

pub fn def(spell: u32) -> Option<&'static SpellDef> {
    SPELLS.iter().find(|s| s.spell == spell)
}

/// One cast's damage: a whole number in the spell's range, `u` in `0..1` picking it.
pub fn roll(def: &SpellDef, u: f32) -> f32 {
    let (lo, hi) = def.damage;
    if hi <= lo {
        return lo as f32;
    }
    let span = (hi - lo + 1) as f32;
    (lo as f32 + (span * u.clamp(0.0, 0.999_999)).floor()).min(hi as f32)
}
```
Run again → PASS.

- [ ] **Step 2: The action machine's cast (failing tests first)**

In `crates/wow-souls/src/combat/action.rs` tests add:

```rust
    fn cast_spec() -> CastSpec {
        CastSpec {
            duration: 1.7,
            release: 0.77,
            recover: 1.23,
        }
    }

    #[test]
    fn a_cast_commits_the_body_until_its_recovery_then_takes_a_roll() {
        let mut a = Action::default();
        assert!(a.begin_cast(133, cast_spec(), None));
        assert!(matches!(a.state, ActionState::Cast(c) if c.spell == 133 && !c.released()));
        let s = full();
        // A roll pressed in the wind-up waits in the buffer; the body stays in the cast.
        assert_eq!(
            step_action(&mut a, &s, input(Some(Press::Evade), true), 0.3),
            Decision::None
        );
        assert!(matches!(a.state, ActionState::Cast(_)));
        // The release passes; still committed.
        step_action(&mut a, &s, input(None, true), 0.5);
        assert!(matches!(a.state, ActionState::Cast(c) if c.released() && !c.cancellable()));
        // From the recovery a press starts the roll.
        assert!(matches!(
            step_action(&mut a, &s, input(Some(Press::Evade), true), 0.5),
            Decision::Evade(_)
        ));
    }

    #[test]
    fn a_cast_ends_at_its_duration_and_a_blow_ends_it_at_once() {
        let mut a = Action::default();
        assert!(a.begin_cast(133, cast_spec(), None));
        assert_eq!(
            step_action(&mut a, &full(), input(None, true), 2.0),
            Decision::Ended
        );
        assert_eq!(a.state, ActionState::Idle);
        assert!(a.begin_cast(133, cast_spec(), None));
        a.stagger(0.4);
        assert!(matches!(a.state, ActionState::Stagger { .. }), "the cast is gone");
    }

    #[test]
    fn a_cast_cannot_start_in_a_swing_that_is_not_yet_cancellable() {
        let mut a = Action::default();
        let swing = Swing {
            t: 0.1,
            kind: MoveKind::R1(0),
            spec: ds_moves().r1[0],
            ds: true,
            serial: 1,
        };
        a.state = ActionState::Attack(swing);
        assert!(!a.can_cast());
        assert!(!a.begin_cast(133, cast_spec(), None));
        assert!(matches!(a.state, ActionState::Attack(_)), "nothing changed");
        a.state = ActionState::Dead;
        assert!(!a.can_cast());
    }

    #[test]
    fn every_cast_has_its_own_serial_so_a_release_happens_once_per_cast() {
        let mut a = Action::default();
        a.begin_cast(133, cast_spec(), None);
        let first = match a.state {
            ActionState::Cast(c) => c.serial,
            _ => unreachable!(),
        };
        a.state = ActionState::Idle;
        a.begin_cast(116, cast_spec(), None);
        let second = match a.state {
            ActionState::Cast(c) => c.serial,
            _ => unreachable!(),
        };
        assert_ne!(first, second);
    }
```
Run: `cargo test -j 4 -p wow-souls combat::action 2>&1 | tail` → FAIL (compile).

- [ ] **Step 3: Implement the cast in the action machine**

In `combat/action.rs` add (after `Reel`):

```rust
/// A spell cast's timing, in seconds from its start (from the Dark Souls cast clip's own flags): when
/// the spell leaves the hand, when the body may act again, and when the clip ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CastSpec {
    pub duration: f32,
    pub release: f32,
    pub recover: f32,
}

/// A cast in progress: the spell, how far in, its timing, its serial (a new cast may release again)
/// and whom it was aimed at when the button was pressed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cast {
    pub t: f32,
    pub spell: u32,
    pub spec: CastSpec,
    pub serial: u32,
    pub target: Option<Entity>,
}

impl Cast {
    /// The spell has left the hand.
    pub fn released(&self) -> bool {
        self.t >= self.spec.release
    }

    /// A buffered action may start now: the recovery has begun.
    pub fn cancellable(&self) -> bool {
        self.t >= self.spec.recover
    }
}
```
`ActionState` gains `/// A spell is being cast: the Dark Souls cast clip poses the body, which is committed to it.\n Cast(Cast),` after `Reel(Reel)`.

In `impl Action`:

```rust
    /// A cast may start now: the body is free, or an action is in its cancel window.
    pub fn can_cast(&self) -> bool {
        self.accepts(Press::Attack)
    }

    /// Starts a cast of `spell`; `false` (nothing changes) when the body cannot take one now.
    pub fn begin_cast(&mut self, spell: u32, spec: CastSpec, target: Option<Entity>) -> bool {
        if !self.can_cast() {
            return false;
        }
        self.swings += 1;
        self.state = ActionState::Cast(Cast {
            t: 0.0,
            spell,
            spec,
            serial: self.swings,
            target,
        });
        self.buffer = None;
        true
    }
```
`accepts`: add `(ActionState::Cast(c), _) => c.cancellable(),` before the `(Stagger | Dead, _)` arm. `step_action`: in the time-advance match `ActionState::Cast(c) => c.t += dt,`; in `free`: `ActionState::Cast(c) => c.cancellable(),`; in `over`: `ActionState::Cast(c) => c.t >= c.spec.duration,`.

In `combat/mod.rs` `act`'s `match action.state` add `ActionState::Cast(_) => *intent = MoveIntent::default(),` (rooted, like a stagger); in the debug-trace match add `ActionState::Cast(c) => format!("cast {} t={:.2}/{:.2}", c.spell, c.t, c.spec.duration),`; re-export `Cast`, `CastSpec` where `Swing`, `Evade` are re-exported (`crate::combat::Swing` is used by tests: find the `pub use action::{...}` line and add `Cast, CastSpec`).

In `dsclip.rs` `playing`:

```rust
        ActionState::Cast(c) => crate::spells::cast::Role::of_spell(c.spell)
            .map(|role| (ClipId::Cast(role), c.t)),
```
Run: `cargo test -j 4 -p wow-souls combat 2>&1 | tail -20` → PASS. Add a `dsclip` test next to `the_action_names_the_clip_it_plays`:

```rust
    #[test]
    fn a_cast_names_its_role_clip() {
        use crate::combat::{Cast, CastSpec};
        let cast = |spell| Cast {
            t: 0.5,
            spell,
            spec: CastSpec { duration: 1.7, release: 0.77, recover: 1.23 },
            serial: 1,
            target: None,
        };
        let state = |spell| Action { state: ActionState::Cast(cast(spell)), ..Action::default() };
        assert_eq!(
            playing(&state(133)),
            Some((ClipId::Cast(crate::spells::cast::Role::Fireball), 0.5))
        );
        assert_eq!(playing(&state(100)), None, "a spell with no cast clip poses nothing");
    }
```

- [ ] **Step 4: The arrow's flight generalized: homing, spell flight, `SpellHit`**

Re-read `crates/wow-souls/src/projectiles.rs` first (co-owned). Failing tests, in its test module:

```rust
    #[test]
    fn steering_turns_toward_the_target_by_at_most_the_allowed_angle_and_keeps_the_speed() {
        let v = Vec3::new(0.0, 0.0, -10.0);
        let to = Vec3::new(10.0, 0.0, 0.0);
        let turned = steer(v, to, 0.2);
        assert!((turned.length() - 10.0).abs() < 1.0e-3, "speed kept");
        assert!((turned.angle_between(v) - 0.2).abs() < 1.0e-3, "turned by the limit");
        assert!(turned.x > 0.0, "toward the target");
        // Within the limit it points straight at the target.
        let near = steer(v, Vec3::new(1.0, 0.0, -10.0), 0.5);
        assert!(near.normalize().angle_between(Vec3::new(1.0, 0.0, -10.0).normalize()) < 1.0e-3);
        // No direction or no speed: untouched.
        assert_eq!(steer(Vec3::ZERO, to, 0.2), Vec3::ZERO);
        assert_eq!(steer(v, Vec3::ZERO, 0.2), v);
    }

    #[test]
    fn a_spell_missile_flies_straight_ahead_when_it_has_no_target() {
        let flight = SpellFlight { spell: 133, life: 1.5, target: None, turn: 2.0, radius: 0.25 };
        let p = Projectile::spell(Vec3::ZERO, Vec3::NEG_Z, 24.0, 18.0, Vec3::ZERO, flight);
        assert_eq!(p.velocity, Vec3::new(0.0, 0.0, -24.0));
        assert_eq!(p.gravity, 0.0);
        assert_eq!(p.damage, 18.0);
        assert!(p.spell.is_some() && !p.bullet);
    }
```
Run → FAIL. Implement: in `projectiles.rs`

```rust
#[derive(Component)]
pub(crate) struct Projectile {
    at: Vec3,
    velocity: Vec3,
    gravity: f32,
    damage: f32,
    from: Vec3,
    age: f32,
    bullet: bool,
    /// A spell's flight: what it is, how long it may fly and what it steers at; `None` for an arrow.
    spell: Option<SpellFlight>,
}

/// What makes a [`Projectile`] a spell's missile.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpellFlight {
    pub spell: u32,
    /// Seconds it may fly (range over speed).
    pub life: f32,
    /// The enemy it steers at (it flies straight when that is dead or gone).
    pub target: Option<Entity>,
    /// How fast it may turn toward the target (rad/s).
    pub turn: f32,
    /// Its body's radius (yd), added to the target's for the contact test.
    pub radius: f32,
}

/// A spell's missile struck `target` at `point`: its extras (slow, burn, the impact) are the spells' to
/// apply ([`crate::spells`]).
#[derive(Message, Debug, Clone, Copy)]
pub struct SpellHit {
    pub target: Entity,
    pub spell: u32,
    pub point: Vec3,
}

impl Projectile {
    /// A spell's missile leaving `at` along `direction` at `speed` (yd/s); it falls under nothing.
    pub(crate) fn spell(
        at: Vec3,
        direction: Vec3,
        speed: f32,
        damage: f32,
        from: Vec3,
        flight: SpellFlight,
    ) -> Projectile {
        Projectile {
            at,
            velocity: direction * speed,
            gravity: 0.0,
            damage,
            from,
            age: 0.0,
            bullet: false,
            spell: Some(flight),
        }
    }
}

/// `velocity` turned toward the direction `to` by at most `max_angle` radians, its speed kept.
pub(crate) fn steer(velocity: Vec3, to: Vec3, max_angle: f32) -> Vec3 {
    let speed = velocity.length();
    let (cur, want) = (velocity.normalize_or_zero(), to.normalize_or_zero());
    if speed == 0.0 || cur == Vec3::ZERO || want == Vec3::ZERO {
        return velocity;
    }
    let angle = cur.angle_between(want);
    if angle <= max_angle {
        return want * speed;
    }
    let axis = cur.cross(want);
    if axis.length_squared() < 1.0e-8 {
        return velocity;
    }
    Quat::from_axis_angle(axis.normalize(), max_angle) * cur * speed
}
```
The existing arrow's construction in `release` (`commands.spawn(Projectile { .. })`) gets `spell: None,`. `contact` gains a `margin` argument (`distance <= r + margin`); the arrow passes `ARROW_MARGIN` (`const ARROW_MARGIN: f32 = 0.06;`), a spell passes its flight radius; update the existing test's calls with `ARROW_MARGIN`. In `fly`: query `shots: Query<(Entity, &mut Projectile, Option<&mut Transform>)>`, add `mut spell_hits: MessageWriter<SpellHit>`; per shot, before the sweep:

```rust
        if let Some(SpellFlight { target: Some(t), turn, .. }) = p.spell {
            if let Ok((_, at, hurt, health, _, _)) = targets.get(t) {
                if health.cur > 0.0 {
                    let (a, b) = hurt.axis(at.translation());
                    p.velocity = steer(p.velocity, (a + b) * 0.5 - p.at, turn * dt);
                }
            }
        }
```
the contact test uses `p.spell.map_or(ARROW_MARGIN, |s| s.radius)`; after the `Hit` write:

```rust
                if let Some(s) = p.spell {
                    spell_hits.write(SpellHit {
                        target,
                        spell: s.spell,
                        point: from + direction * limit,
                    });
                }
```
the end-of-flight condition becomes `nearest.is_some() || wall.is_some() || p.age > p.spell.map_or(4.0, |s| s.life)`, and after `p.at = to;` move the model: `if let Some(mut t) = xf { t.translation = to; if direction != Vec3::ZERO { t.look_to(direction, Vec3::Y); } }`. In `plugin`: `app.add_message::<SpellHit>()`.

Run: `cargo test -j 4 -p wow-souls projectiles 2>&1 | tail` → PASS.

- [ ] **Step 5: `spells`: the plan, the instant aim (failing tests first)**

In `crates/wow-souls/src/spells/mod.rs` add tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wow_world::spells;

    fn timing() -> cast::Timing {
        cast::Timing { duration: 1.733, release: 0.767, recover: 1.233 }
    }
    fn foe(n: u32) -> Entity {
        Entity::from_raw_u32(n).unwrap()
    }

    #[test]
    fn a_missile_goes_at_the_locked_enemy_or_at_nothing() {
        let f = spells::def(133).unwrap();
        let p = plan(f, true, true, Some(timing()), Vec3::ZERO, Vec3::NEG_Z, Some(foe(3)), &[]).unwrap();
        assert_eq!(p.target, Some(foe(3)));
        assert_eq!(p.spec, CastSpec { duration: 1.733, release: 0.767, recover: 1.233 });
        let p = plan(f, true, true, Some(timing()), Vec3::ZERO, Vec3::NEG_Z, None, &[]).unwrap();
        assert_eq!(p.target, None, "it flies ahead");
    }

    #[test]
    fn without_cast_clips_or_a_free_body_the_cast_is_refused() {
        let f = spells::def(133).unwrap();
        assert_eq!(
            plan(f, true, true, None, Vec3::ZERO, Vec3::NEG_Z, None, &[]),
            Err(Refusal::NotYet),
            "no Dark Souls cache"
        );
        assert_eq!(
            plan(f, false, true, Some(timing()), Vec3::ZERO, Vec3::NEG_Z, None, &[]),
            Err(Refusal::Busy)
        );
        assert_eq!(
            plan(f, true, false, Some(timing()), Vec3::ZERO, Vec3::NEG_Z, None, &[]),
            Err(Refusal::Busy),
            "not in the air"
        );
    }

    #[test]
    fn an_instant_spell_needs_a_target_in_range_before_it_is_paid_for() {
        let f = spells::def(2136).unwrap();
        let near = [(foe(1), Vec3::new(0.0, 0.0, -10.0))];
        let far = [(foe(2), Vec3::new(0.0, 0.0, -30.0))];
        let ok = plan(f, true, true, Some(timing()), Vec3::ZERO, Vec3::NEG_Z, None, &near).unwrap();
        assert_eq!(ok.target, Some(foe(1)), "the nearest enemy ahead");
        assert_eq!(
            plan(f, true, true, Some(timing()), Vec3::ZERO, Vec3::NEG_Z, None, &far),
            Err(Refusal::TooFar)
        );
        assert_eq!(
            plan(f, true, true, Some(timing()), Vec3::ZERO, Vec3::NEG_Z, None, &[]),
            Err(Refusal::NoTarget)
        );
        // Behind the caster is not ahead.
        let behind = [(foe(4), Vec3::new(0.0, 0.0, 5.0))];
        assert_eq!(
            plan(f, true, true, Some(timing()), Vec3::ZERO, Vec3::NEG_Z, None, &behind),
            Err(Refusal::NoTarget)
        );
        // The locked enemy counts wherever it is, if it is in range.
        let ok = plan(f, true, true, Some(timing()), Vec3::ZERO, Vec3::NEG_Z, Some(foe(4)), &behind).unwrap();
        assert_eq!(ok.target, Some(foe(4)));
    }

    #[test]
    fn a_buff_needs_no_target() {
        let f = spells::def(168).unwrap();
        let p = plan(f, true, true, Some(timing()), Vec3::ZERO, Vec3::NEG_Z, None, &[]).unwrap();
        assert_eq!(p.target, None);
    }

    #[test]
    fn a_release_happens_once_per_cast() {
        let mut fired = None;
        assert!(release_due(&mut fired, foe(1), 5, true));
        assert!(!release_due(&mut fired, foe(1), 5, true), "same cast, same release");
        assert!(release_due(&mut fired, foe(1), 6, true), "the next cast releases again");
        assert!(!release_due(&mut fired, foe(1), 7, false), "not yet released");
    }
}
```
Run → FAIL.

- [ ] **Step 6: `spells/mod.rs`: plan, aim, release**

Replace the file with (keeping `pub mod cast;`):

```rust
//! The Mage's spells (spec `docs/superpowers/specs/2026-10-14-mage-class-design.md`): checks a cast
//! before it is paid for ([`plan`]), the release (a missile, an instant hit or a buff), the missile's
//! hits and what they leave behind (a slow, a burn), the buffs.

pub mod cast;

use bevy::prelude::*;

use wow_world::spells::{self as defs, Delivery, SpellDef};

use crate::abilities::fx::{self, Kind};
use crate::abilities::{Buffs, Refusal};
use crate::actor::held::HeldBuilt;
use crate::audio::Audio;
use crate::avatar::Avatar;
use crate::combat::{Action, ActionState, CastSpec, Health};
use crate::hits::{Hit, Hurtbox};
use crate::lockon::{LockOn, Lockable};
use crate::npc::Hostile;
use crate::projectiles::{Projectile, SpellFlight, SpellHit};
use crate::settle::Phase;

/// An enemy this far off the way ahead (cosine of the angle: 60 degrees) is not the one an instant spell
/// picks when none is locked.
const AIM_CONE: f32 = 0.5;
/// How fast a missile may turn per degree of the Dark Souls bullet's homing angle (rad/s per degree).
const TURN_PER_HOMING_DEG: f32 = 0.105;
/// Poise damage and flinch level of a spell's blow (a bow's arrow: 8, level 1).
const SPELL_POISE: f32 = 8.0;

/// What a checked cast will do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CastPlan {
    pub spec: CastSpec,
    pub target: Option<Entity>,
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// The enemy an instant spell hits: the locked one (any direction) or the nearest ahead, within `range`.
pub fn aim_instant(
    player: Vec3,
    facing: Vec3,
    locked: Option<Entity>,
    foes: &[(Entity, Vec3)],
    range: f32,
) -> Result<Entity, Refusal> {
    let reach = |p: Vec3| flat(p - player).length();
    let judge = |d: f32| if d > range { Err(Refusal::TooFar) } else { Ok(()) };
    if let Some((e, p)) = locked.and_then(|l| foes.iter().find(|(e, _)| *e == l)) {
        return judge(reach(*p)).map(|()| *e);
    }
    let nose = flat(facing).normalize_or_zero();
    let mut ahead: Vec<(Entity, f32)> = foes
        .iter()
        .filter(|(_, p)| flat(*p - player).normalize_or_zero().dot(nose) >= AIM_CONE)
        .map(|(e, p)| (*e, reach(*p)))
        .collect();
    ahead.sort_by(|a, b| a.1.total_cmp(&b.1));
    match ahead.first() {
        Some((e, d)) => judge(*d).map(|()| *e),
        None => Err(Refusal::NoTarget),
    }
}

/// Checks a cast before anything is paid: the cast clips are loaded, the body can start one (`can_cast`,
/// `grounded`), and an instant spell has its target in range.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    def: &SpellDef,
    can_cast: bool,
    grounded: bool,
    timing: Option<cast::Timing>,
    player: Vec3,
    facing: Vec3,
    locked: Option<Entity>,
    foes: &[(Entity, Vec3)],
) -> Result<CastPlan, Refusal> {
    let t = timing.ok_or(Refusal::NotYet)?;
    if !can_cast || !grounded {
        return Err(Refusal::Busy);
    }
    let target = match def.delivery {
        Delivery::Instant => Some(aim_instant(player, facing, locked, foes, def.range)?),
        Delivery::Missile { .. } => locked,
        Delivery::SelfBuff => None,
    };
    Ok(CastPlan {
        spec: CastSpec {
            duration: t.duration,
            release: t.release,
            recover: t.recover,
        },
        target,
    })
}

/// A cast's release fires once: `true` the first time a released cast of this `serial` is seen.
pub fn release_due(
    fired: &mut Option<(Entity, u32)>,
    who: Entity,
    serial: u32,
    released: bool,
) -> bool {
    if !released || *fired == Some((who, serial)) {
        return false;
    }
    *fired = Some((who, serial));
    true
}

/// Uniform numbers for the damage rolls (xorshift; seeded from the clock).
#[derive(Resource)]
pub struct SpellRng(u32);

impl Default for SpellRng {
    fn default() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x9E37_79B9, |d| d.subsec_nanos() ^ 0x9E37_79B9);
        SpellRng(seed.max(1))
    }
}

impl SpellRng {
    pub fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// The spell leaves the hand: a missile, an instant hit or a buff. Runs in `PostUpdate` after the
/// body's bones are placed, as the arrow's release does, so the missile starts at the hand.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn release(
    mut commands: Commands,
    mut buffs: ResMut<Buffs>,
    mut rng: ResMut<SpellRng>,
    kit: Option<Res<cast::Kit>>,
    fx: Option<Res<fx::Fx>>,
    audio: Option<ResMut<Audio>>,
    mut hits: MessageWriter<Hit>,
    mut spell_hits: MessageWriter<SpellHit>,
    avatar: Query<(Entity, &Action, &GlobalTransform, Option<&HeldBuilt>), With<Avatar>>,
    roots: Query<&GlobalTransform>,
    mut foes: Query<(&GlobalTransform, &Lockable, &Hurtbox, &mut Health), (With<Hostile>, Without<Avatar>)>,
    lock: Res<LockOn>,
    mut fired: Local<Option<(Entity, u32)>>,
) {
    let Ok((me, action, body, held)) = avatar.single() else {
        return;
    };
    let ActionState::Cast(cast) = action.state else {
        return;
    };
    if !release_due(&mut fired, me, cast.serial, cast.released()) {
        return;
    }
    let (Some(def), Some(fx)) = (defs::def(cast.spell), fx) else {
        return;
    };
    let mut audio = audio.map(|a| a.into_inner());
    let at = held.and_then(|h| roots.get(h.root).ok()).map_or(
        body.translation() + Vec3::Y * 1.4,
        GlobalTransform::translation,
    );
    let fire = def.school == defs::School::Fire;
    for hand in [0x15, 0x16] {
        fx::spawn(&mut commands, &fx, if fire { Kind::FireHand(hand) } else { Kind::IceHand(hand) }, me);
    }
    fx::sound(audio.as_deref_mut(), if fire { fx::SOUND_FIRE_CAST } else { fx::SOUND_ICE_CAST }, "spell cast");
    let damage = defs::roll(def, rng.next());
    match def.delivery {
        Delivery::SelfBuff => {
            buffs.frost_armor = def.secs;
            fx::spawn(&mut commands, &fx, Kind::FrostArmor, me);
            fx::sound(audio.as_deref_mut(), fx::SOUND_FROST_ARMOR, "frost armor");
        }
        Delivery::Instant => {
            let Some(target) = cast.target else {
                return;
            };
            let Ok((tf, _, hurt, mut health)) = foes.get_mut(target) else {
                return;
            };
            if health.cur <= 0.0 {
                return;
            }
            health.cur = (health.cur - damage).max(0.0);
            let (a, b) = hurt.axis(tf.translation());
            let point = (a + b) * 0.5;
            hits.write(Hit {
                target,
                damage,
                point,
                lethal: health.cur <= 0.0,
                level: 1,
                poise: SPELL_POISE,
                from: body.translation(),
                knock: 0.0,
            });
            spell_hits.write(SpellHit {
                target,
                spell: cast.spell,
                point,
            });
        }
        Delivery::Missile { speed } => {
            let role = cast::Role::of_spell(cast.spell);
            let homing = role
                .and_then(|r| kit.as_deref().and_then(|k| k.clip(r)))
                .and_then(|c| c.bullet)
                .map_or(0.0, |b| b.homing_deg);
            let radius = role
                .and_then(|r| kit.as_deref().and_then(|k| k.clip(r)))
                .and_then(|c| c.bullet)
                .map_or(0.2, |b| b.radius * crate::shove::METRES_TO_YARDS);
            let target = cast
                .target
                .or(lock.target)
                .filter(|t| foes.get(*t).is_ok_and(|(_, _, _, h)| h.cur > 0.0));
            let direction = target
                .and_then(|t| foes.get(t).ok())
                .map(|(tf, l, _, _)| (tf.translation() + Vec3::Y * l.aim_height - at).normalize_or(*body.forward()))
                .unwrap_or(*body.forward());
            let flight = SpellFlight {
                spell: cast.spell,
                life: def.range / speed,
                target,
                turn: homing * TURN_PER_HOMING_DEG,
                radius,
            };
            let missile = commands
                .spawn((
                    Name::new("spell missile"),
                    Projectile::spell(at, direction, speed, damage, body.translation(), flight),
                    Transform::from_translation(at),
                    Visibility::default(),
                ))
                .id();
            let model = if fire { Kind::FireballMissile } else { Kind::FrostboltMissile };
            fx::spawn(&mut commands, &fx, model, missile);
            info!("spells: {} released, {damage:.0} damage, homing {homing:.0} deg", cast.spell);
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<SpellRng>()
        .add_systems(Startup, cast::load_kit)
        .add_systems(
            PostUpdate,
            release.after(benilla_world::billboard::BillboardPlace),
        )
        .add_systems(Update, spell_hit.run_if(in_state(Phase::Playing)));
}
```
The helpers `normalize_or` and `METRES_TO_YARDS` exist (`projectiles.rs` uses `normalize_or`, `dsclip` uses `crate::shove::METRES_TO_YARDS`). The plugin names `spell_hit`, which Step 8 writes: the module compiles only after Step 8, so run the tests of Step 5 in the order Steps 5, 6, 7, 8 and gate once at Step 9.

- [ ] **Step 7: The cast starts from the hotbar; the WoW looks (fx.rs)**

In `abilities/fx.rs` add the sound constants and kinds from the `spellvis` readings (Fireball 133 / Frostbolt 116 / Fire Blast 2136 / Frost Armor 168):

```rust
pub const SOUND_FIRE_PRECAST: u32 = 702;
pub const SOUND_FIRE_CAST: u32 = 1484;
pub const SOUND_FIRE_IMPACT: u32 = 1507;
pub const SOUND_ICE_PRECAST: u32 = 20;
pub const SOUND_ICE_CAST: u32 = 2559;
pub const SOUND_ICE_IMPACT: u32 = 7;
pub const SOUND_FROST_ARMOR: u32 = 1456;
```
`Kind` gains: `FireballMissile`, `FirePrecastHand(u16)`, `FireHand(u16)`, `FireImpact`, `FireBlastImpact`, `FrostboltMissile`, `IceHand(u16)`, `IceImpact`, `IceMark`, `FrostArmor`, with

| Kind | path | attach |
|---|---|---|
| FireballMissile | `Spells\Fireball_Missile_Low.mdx` | `None` |
| FirePrecastHand(id) | `Spells\Fire_Precast_Hand.mdx` | `Some(id)` |
| FireHand(id) | `Spells\Fire_Cast_Hand.mdx` | `Some(id)` |
| FireImpact | `Spells\MoltenBlast_Impact_Chest.mdx` | `Some(0x22)` |
| FireBlastImpact | `Spells\Fire_ImpactDD_Chest.mdx` | `Some(0x22)` |
| FrostboltMissile | `Spells\Frostbolt.mdx` | `None` |
| IceHand(id) | `Spells\Ice_Precast_Low_Hand.mdx` | `Some(id)` |
| IceImpact | `Spells\Ice_ImpactDD_Med_Chest.mdx` | `Some(0x22)` |
| IceMark | `Spells\Ice_Precast_Uber_Head.mdx` | `Some(0x14)` |
| FrostArmor | `Spells\FrostArmor_Low_Head.mdx` | `Some(0x14)` |

Add them to `ALL` (length grows: `[Kind; 8]` becomes `[Kind; 20]`: the hands at `0x15` and `0x16` for each of the three hand kinds), to `name()`, and add the new sound ids to the install test's `for kit in [...]` list. The existing install test then verifies every model and sound kit is in the client.

In `abilities/mod.rs` `input`:

1. The avatar query becomes `Query<(Entity, &Transform, &mut Action, &Health, &crate::motor::Motor), With<Avatar>>` and the destructuring `let Ok((me, xf, mut action, health, motor)) = avatar.single_mut() else { return };`. The early-return line uses `action.is_dead()` (works on the `Mut`).
2. Add `casts: Option<Res<crate::spells::cast::Kit>>` to a parameter tuple (with `toasts`, `claps`, `audio`).
3. Before `match activate(...)`, for a spell ability make the plan and refuse early:

```rust
        // A spell is a committed cast: its clips, a free body and (instant) a target are checked
        // before anything is paid.
        let cast_plan = match wow_world::spells::def(spell) {
            None => None,
            Some(def) => {
                let ahead: Vec<(Entity, Vec3)> = foes
                    .iter()
                    .filter(|(_, _, reaction)| **reaction != Reaction::Dead)
                    .map(|(e, at, _)| (e, at.translation()))
                    .collect();
                let timing = casts.as_deref().and_then(|k| k.timing(spell));
                match crate::spells::plan(
                    def,
                    action.can_cast(),
                    motor.grounded,
                    timing,
                    xf.translation,
                    *xf.forward(),
                    lock.target,
                    &ahead,
                ) {
                    Ok(p) => Some(p),
                    Err(why) => {
                        info!("abilities: {} refused: {}", ability.name, why.message());
                        toasts.push(now, why.message());
                        continue;
                    }
                }
            }
        };
```
4. The `Cast::Spell` arm:

```rust
                    Cast::Spell(spell) => {
                        if let Some(p) = cast_plan {
                            action.begin_cast(spell, p.spec, p.target);
                            if let Some(def) = wow_world::spells::def(spell) {
                                let fire = def.school == wow_world::spells::School::Fire;
                                for hand in [0x15, 0x16] {
                                    let kind = if fire { fx::Kind::FirePrecastHand(hand) } else { fx::Kind::IceHand(hand) };
                                    fx::spawn(&mut commands, &fx, kind, me);
                                }
                                fx::sound(
                                    audio.as_deref_mut(),
                                    if fire { fx::SOUND_FIRE_PRECAST } else { fx::SOUND_ICE_PRECAST },
                                    "spell precast",
                                );
                            }
                        }
                    }
```
The abilities' `usable` flags flip per spell as each lands; Fireball first: in `wow-world/src/abilities.rs` set Fireball's `usable: true` and update the test `the_mage_knows_fireball_and_frost_armor_and_learns_the_other_two` (the all-unusable assertion becomes "all but Fireball", then "all but Fireball and Frostbolt" in Task 5...). Keep each test honest to the step.

- [ ] **Step 8: The missile's hit: impact and sound (and Task 5's extras later)**

Append to `spells/mod.rs`:

```rust
/// What a missile's hit leaves: the impact model and sound on the target (slow and burn come in the
/// next task).
fn spell_hit(
    mut commands: Commands,
    mut hits: MessageReader<SpellHit>,
    fx: Option<Res<fx::Fx>>,
    audio: Option<ResMut<Audio>>,
) {
    let Some(fx) = fx else {
        hits.clear();
        return;
    };
    let mut audio = audio.map(|a| a.into_inner());
    for h in hits.read() {
        let Some(def) = defs::def(h.spell) else {
            continue;
        };
        let fire = def.school == defs::School::Fire;
        let kind = match (def.spell, fire) {
            (2136, _) => Kind::FireBlastImpact,
            (_, true) => Kind::FireImpact,
            (_, false) => Kind::IceImpact,
        };
        fx::spawn(&mut commands, &fx, kind, h.target);
        fx::sound(
            audio.as_deref_mut(),
            if fire { fx::SOUND_FIRE_IMPACT } else { fx::SOUND_ICE_IMPACT },
            "spell impact",
        );
    }
}
```
`Buffs` needs `pub frost_armor: f32` for `release` to compile: add the field now (Task 5 wires its effect): `pub struct Buffs { pub shout: f32, pub frost_armor: f32 }`; `Buffs::default()` still all-zero; `upkeep` decrements it (`buffs.frost_armor = (buffs.frost_armor - dt).max(0.0);`).

- [ ] **Step 9: Run the gate**

Run the Task 1 Step 8 gate. Expected: all green. Fix compile errors by following the compiler: every `match` on `ActionState` outside the files listed gets `Cast` handled the same way as `Attack` where it asks "is the body busy".

- [ ] **Step 10: Fireball, live, in motion**

```bash
cd /c/Spel/wow-souls && cargo build -j 4 -p wow-souls && ./target/debug/wow-souls-dsr.exe casts | tail -3
S=$(mktemp -d) && printf 'wschar 2\nname Merlin\nrace human\nclass mage\nsex male\nskin 0\nface 0\nhair_style 0\nhair_color 0\nfacial_hair 0\n' > "$S/character-v1.txt"
WOWSOULS_CHARACTER="$S/character-v1.txt" WOWSOULS_CLOCK_SAVE="$S/clock.txt" WOWSOULS_ITEMS_SAVE="$S/items.txt" WOWSOULS_QUESTS_SAVE="$S/quests.txt" WOWSOULS_SOULS_SAVE="$S/souls.txt" \
  WOWSOULS_LOCK=5 WOWSOULS_ABILITY_SCRIPT=6.5:1 WOWSOULS_SHOT="$S/fb.png" WOWSOULS_SHOT_AT=6.4,6.6,6.8,7.0,7.2,7.4,7.6,7.8,8.0,8.3 WOWSOULS_CHECK=10 \
  timeout 120 ./target/debug/wow-souls.exe 2>&1 | grep -i "abilities\|spells\|projectile\|hit\|slain" | head -30; ls "$S"
```
(Stand where a kobold is within 35 yd: use the position and lock recipe in `docs/superpowers/specs/` / the archive entry "abilities check" (`grep -n 'ability-check' docs/log/INDEX.txt`); a kobold ~16 yd ahead with `WOWSOULS_LOCK=5` is the existing recipe for Charge.) Expected log: `abilities: Fireball (rage ...)`, `spells: 133 released, NN damage, homing 10 deg`, a hit on the kobold. Read the ten frames: the cast pose rises and releases, a fire ball leaves the hand at the release frame, reaches the enemy, the impact shows, the kobold flinches. Judge the animation in motion (not one still) and write what you saw per frame in the log. The cast must not move the body and a hit during the wind-up (let a kobold reach you) must cancel it with no missile.

- [ ] **Step 11: Interrupt check**

Run the same with a second script: `WOWSOULS_ABILITY_SCRIPT=6.5:1,7.0:1` and a kobold at melee range hitting the player; the log must show `combat: ... reel` or `stagger` while `Cast`, then no `released` line for that cast, and `mana` stays spent (the next press says "Not enough mana." only when it is really short). Then the stale-target check: cast two Fireballs a second apart at one kobold with 1 HP left (`WOWSOULS_ABILITY_SCRIPT=6.5:1,7.3:1`): the first kills it while the second is in flight; the log must show the kill, no panic, and the second missile ending at its range (`spells: ... released` then silence, no `hit`). Record in the log.

- [ ] **Step 12: Update the spec and HANDOVER**

In `docs/superpowers/specs/2026-10-14-mage-class-design.md` section 3, replace the bullet "**Release**: ..." with the as-built flight: WoW's speed (24 / 28 yd/s) and range (35 / 30 yd) from `spell_template` and `SpellRange`, the DS `Bullet` row's homing angle and hit radius (turn rate = angle x 0.105 rad/s per degree, first guess), no gravity (DS's Fireball is a 10-m lob that cannot reach 35 yd), and add to "What exists" nothing. HANDOVER item 1: "S4 done: casting Fireball end to end; Frostbolt / Fire Blast / Frost Armor next". No commit.

---

### Task 5: Frostbolt, Fire Blast, Frost Armor (S5)

**Files:**
- Modify: `crates/wow-souls/src/abilities/moves.rs` (`Slowed.factor`), `crates/wow-souls/src/spells/mod.rs`, `crates/wow-world/src/gear_effect.rs`, `crates/wow-souls/src/gear_effect.rs`, `crates/wow-world/src/abilities.rs` (`usable`)
- Test: those files' test modules

**Interfaces:**
- Consumes: Task 4's `SpellHit`, `release`, `Buffs.frost_armor`, `defs::{Dot, Slow}`.
- Produces: `moves::Slowed { left, factor }`; `spells::{Burning, burn_tick}`; `GearEffect.buff_armor: u32`.

- [ ] **Step 1: Failing tests: a slow has its own factor; a burn ticks; armor buff**

In `abilities/moves.rs` the `Slowed` doc test is implicit; add a pure function and test:

```rust
    #[test]
    fn a_slow_scales_speed_by_its_own_factor() {
        assert_eq!(slowed_speed(7.0, 0.6), 4.2, "Thunder Clap: 40 % slower");
        assert!((slowed_speed(7.0, 0.58) - 4.06).abs() < 1.0e-5, "Frostbolt: 42 % slower");
        assert_eq!(slowed_speed(7.0, 1.0), 7.0);
    }
```
In `spells/mod.rs` tests:

```rust
    #[test]
    fn a_burn_ticks_every_two_seconds_twice_and_ends() {
        let mut b = Burning { per_tick: 1.0, ticks_left: 2, every: 2.0, timer: 2.0 };
        assert_eq!(burn_tick(&mut b, 1.9), 0.0);
        assert_eq!(burn_tick(&mut b, 0.2), 1.0, "the first tick at 2 s");
        assert_eq!(burn_tick(&mut b, 2.0), 1.0, "the second at 4 s");
        assert!(b.ticks_left == 0);
        assert_eq!(burn_tick(&mut b, 5.0), 0.0, "it is over");
    }
```
In `wow-world/src/gear_effect.rs` tests:

```rust
    #[test]
    fn a_buff_adds_to_the_armor_the_formula_reads() {
        let mut fx = GearEffect::default();
        assert_eq!(fx.reduction(3), 0.0);
        fx.buff_armor = 30;
        let want = armor_reduction(30, 3);
        assert!((fx.reduction(3) - want).abs() < 1.0e-6);
        assert!(want > 0.05 && want < 0.08, "30 armor at level 3 is about 7 %: {want}");
    }
```
Run → FAIL (compile).

- [ ] **Step 2: Implement**

`moves.rs`:

```rust
/// A slowed enemy: the seconds left and the share of its speed it keeps.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Slowed {
    pub left: f32,
    pub factor: f32,
}

/// A walking speed under a slow of `factor`.
pub fn slowed_speed(speed: f32, factor: f32) -> f32 {
    speed * factor
}
```
`thunder_clap` inserts `Slowed { left: CLAP_SLOW_SECS, factor: CLAP_SLOW }`; `apply_slow`: `intent.speed = slowed_speed(intent.speed, slowed.factor);` and `*left += dt * (1.0 - slowed.factor);` (read `slowed.factor` before the `if let Mind::Cooldown` borrow).

`wow-world/gear_effect.rs`: `GearEffect` gains `pub buff_armor: u32` (default 0 in `Default` and `from_gear`), `reduction` reads `armor_reduction(self.armor + self.buff_armor, attacker_level)`, `summary` unchanged. The game's `gear_effect::refresh` keeps the buff across recomputes: after `now` is computed add `now.buff_armor = fx.0.buff_armor;`.

`spells/mod.rs`:

```rust
/// A fire's burn on an enemy: `per_tick` damage `ticks_left` more times, one every `every` seconds.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Burning {
    pub per_tick: f32,
    pub ticks_left: u32,
    pub every: f32,
    pub timer: f32,
}

/// `dt` seconds pass: the damage of a tick that fell due (0 when none did).
pub fn burn_tick(b: &mut Burning, dt: f32) -> f32 {
    if b.ticks_left == 0 {
        return 0.0;
    }
    b.timer -= dt;
    if b.timer > 0.0 {
        return 0.0;
    }
    b.timer += b.every;
    b.ticks_left -= 1;
    b.per_tick
}

/// Burns tick. A tick that does not kill is silent (no flinch); the one that does carries a [`Hit`]
/// so the kill pays out (XP, souls, loot) like any other.
fn burn(
    mut commands: Commands,
    time: Res<Time>,
    mut hits: MessageWriter<Hit>,
    avatar: Query<&GlobalTransform, With<Avatar>>,
    mut burning: Query<(Entity, &mut Burning, &mut Health, &GlobalTransform), With<Hostile>>,
) {
    let from = avatar.single().map_or(Vec3::ZERO, GlobalTransform::translation);
    for (e, mut b, mut health, at) in &mut burning {
        let damage = burn_tick(&mut b, time.delta_secs());
        if health.cur > 0.0 && damage > 0.0 {
            health.cur = (health.cur - damage).max(0.0);
            if health.cur <= 0.0 {
                hits.write(Hit {
                    target: e,
                    damage,
                    point: at.translation() + Vec3::Y * 0.9,
                    lethal: true,
                    level: 0,
                    poise: 0.0,
                    from,
                    knock: 0.0,
                });
            }
        }
        if b.ticks_left == 0 || health.cur <= 0.0 {
            commands.entity(e).remove::<Burning>();
        }
    }
}

/// The armor buff follows its seconds.
fn frost_armor(
    buffs: Res<Buffs>,
    mut gear: ResMut<crate::gear_effect::GearFx>,
) {
    let want = if buffs.frost_armor > 0.0 {
        defs::def(168).map_or(0, |d| d.armor)
    } else {
        0
    };
    if gear.0.buff_armor != want {
        gear.0.buff_armor = want;
        info!("spells: frost armor {}", if want > 0 { "on" } else { "off" });
    }
}
```
Extend `spell_hit` (Task 4 Step 8) after the impact: for an alive target (`foes: Query<&Health, With<Hostile>>`), insert the slow and burn:

```rust
        let alive = foes.get(h.target).is_ok_and(|hp| hp.cur > 0.0);
        if alive {
            if let Some(slow) = def.slow {
                commands.entity(h.target).insert(Slowed { left: slow.secs, factor: 1.0 - slow.share });
                fx::spawn(&mut commands, &fx, Kind::IceMark, h.target);
            }
            if let Some(dot) = def.dot {
                commands.entity(h.target).insert(Burning {
                    per_tick: dot.per_tick,
                    ticks_left: dot.ticks,
                    every: dot.every,
                    timer: dot.every,
                });
            }
        }
```
(imports `crate::abilities::moves::Slowed` via the `abilities` re-export; the system gains `foes: Query<&Health, With<Hostile>>`.) Register `burn` and `frost_armor` in the plugin's `Update` set. Set `usable: true` for Frostbolt, Fire Blast and Frost Armor in `ALL`, and make the mage-ability test assert all four usable.

- [ ] **Step 3: Gate, then each spell in motion**

Run the Task 1 Step 8 gate. Then three frame series as in Task 4 Step 10 with `WOWSOULS_MANA=300` (so mana does not interfere): Frostbolt (`WOWSOULS_ABILITY_SCRIPT=6.5:3` after `WOWSOULS_ITEMS_SAVE` is seeded with the learned spell: use the scripted trainer, or start with `abilities` learned via a scratch `items.txt` containing the ability line — read `saves.rs` for the `ability` line format), Fire Blast (slot 4), Frost Armor (slot 2): check in each: the cast plays its DS clip, the WoW hand effects glow, the spell lands (Frostbolt: the kobold visibly slows for 5 s; Fire Blast: instant hit, 8 s cooldown number on the icon; Frost Armor: the head effect and the armor line in the log `gear: armor 30 ...`). Note any clip that looks wrong in the hand; swap it per Task 3 Step 5's rule and regenerate with `wow-souls-dsr casts`.

- [ ] **Step 4: Log and HANDOVER**

`docs/log/2026-10-14-mage-s5.md` with the verdicts; HANDOVER item 1: "S5 done: all four spells work". No commit.

---

### Task 6: Trainer, hotbar, the Mage's day (S6)

**Files:**
- Modify: `crates/wow-souls/src/shop/mod.rs:302-330,589-597`, `crates/wow-souls/src/quests/mod.rs:786`, `README.md`, `docs/HANDOVER.md`
- Test: `crates/wow-souls/src/shop/mod.rs` tests

**Interfaces:**
- Consumes: `class::{def, trainer_of}`, `abilities::trainer_list`.
- Produces: `shop::services(data, flags, entry, class: u8)`.

- [ ] **Step 1: Failing tests: the class gate**

In `shop/mod.rs` tests add:

```rust
    #[test]
    fn a_trainer_teaches_only_its_own_class() {
        // Llane Beshere (911, warrior) and Khelden Bremen (198, mage): npc_flags 19 has the trainer bit.
        assert!(services(None, 19, 911, 1).trainer);
        assert!(!services(None, 19, 911, 8).trainer, "a mage is not taught by the warrior trainer");
        assert!(services(None, 19, 198, 8).trainer);
        assert!(!services(None, 19, 198, 1).trainer);
        assert!(!services(None, 19, 152, 8).trainer, "Brother Danil teaches nothing");
    }

    #[test]
    fn a_forged_lesson_of_another_trainer_is_refused() {
        let mut c = ctx_with_souls(5000);
        c.npc = 911;
        c.level = 9;
        // Frostbolt is the mage trainer's, not Llane Beshere's.
        assert_eq!(
            attempt(Request::Train(116), &c).unwrap_err(),
            Refused::Train(abilities::Refusal::NotYet)
        );
        c.npc = 198;
        // The trainer's own list is accepted (Frostbolt is usable once Task 5 has landed).
        assert!(attempt(Request::Train(116), &c).is_ok());
    }
```
(`ctx_with_souls` is the file's existing test helper that builds a `Ctx`; if it has another name, use the helper the neighbouring train tests (lines ~715-745) use.)

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -j 4 -p wow-souls shop 2>&1 | tail -15` → FAIL (signature).

- [ ] **Step 3: Implement**

```rust
/// What an NPC offers besides quests: wares (the vendor flag and a stock row), the strongbox (a banker),
/// lessons (the trainer flag and a list of the character's own class).
pub fn services(data: Option<&Data>, flags: u32, entry: u32, class: u8) -> Services {
    Services {
        vendor: data.is_some_and(|d| flags & 4 != 0 && d.0.vendors.iter().any(|v| v.npc == entry)),
        banker: BANKERS.contains(&entry),
        trainer: flags & 16 != 0
            && wow_world::class::trainer_of(entry).is_some_and(|c| c.id == class)
            && !abilities::trainer_list(entry).is_empty(),
    }
}
```
and in `attempt`, `Request::Train(spell)` first:

```rust
        Request::Train(spell) => {
            // Only what this trainer's list offers: a request for another class's lesson is a forgery.
            if !abilities::trainer_list(c.npc).iter().any(|a| a.spell == spell) {
                return Err(Refused::Train(abilities::Refusal::NotYet));
            }
            let ability =
                abilities::find(spell).ok_or(Refused::Train(abilities::Refusal::NotYet))?;
            ...
```
`quests/mod.rs:786`: `crate::shop::services(loot_data.as_deref(), n.flags, n.entry, g.player.class as u8)`.

- [ ] **Step 4: Gate**

Run the Task 1 Step 8 gate. Expected: green.

- [ ] **Step 5: The Mage's first minutes, live**

A scripted run of a mage at level 4 with 400 souls (`WOWSOULS_SOULS_SCRIPT=2:give:400`, `WOWSOULS_QUESTS_SCRIPT=1:xp:...` to reach level 4; the XP-for-level table is in `docs/log` under "xp"), at Khelden Bremen in the Abbey (use the Abbey position the shop checks use: `grep -n 'WOWSOULS_AT' docs/log/INDEX.txt` for "shops"), `WOWSOULS_SHOP_SCRIPT` confirming "Frostbolt": the log must show `shop: ... learned Frostbolt for 400 souls`, then the hotbar gains a third slot and the HUD shot shows three icons with the mana bar. Read the frames and describe them in the log.

- [ ] **Step 6: Docs**

`README.md`: add to the abilities section a "Mage" paragraph (the four spells, the keys 1-4 as the warrior's, `WOWSOULS_MANA=<n>`, casts committed and cancelled by a blow, the trainer in the Abbey) and `WOWSOULS_MANA` to the switch table. `docs/HANDOVER.md` State "Player" line: "classes: Warrior and Mage (Fireball, Frost Armor, Frostbolt, Fire Blast; DS cast clips; mana)"; Next item 1 becomes "Mage S1-S6 built; the user judges the cast in motion; S7 (drink Refreshing Spring Water, balance) gets its own plan after that". Keep the file under 8 KB (`wc -c docs/HANDOVER.md`).

- [ ] **Step 7: Final gate and report**

Run the full gate with the installs set (Task 1 Step 8). Report counts, the frame verdicts, and what the user should judge: the cast length (1.7 s) in a Souls fight, the mana pool and its trickle, the clip picks. No commit; ask the user whether to commit when they have played it.

---

## Self-review

**Spec coverage:** class row (Task 1); mana, regen, bar, level/gear follow (Task 2); cast clips, `casts.txt`, survey, `ClipId::Cast`, DSVIEW (Task 3); `ActionState::Cast`, interrupt, roll cancel from recovery, missile with homing, WoW damage rolls, release timing, WoW visuals and sounds, Fireball (Task 4); Frostbolt slow, Fire Blast instant with pre-checked target, Frost Armor armor, burn (Task 5); trainer with class gate, hotbar, docs (Task 6); "Not in v1" items (drink, Conjure, Arcane Intellect, wand, ranks, chill, quests-specific, other races) have no task, S7 is a separate plan. Spec risk 5 (quests' class masks) is handled by `Player::of_class` feeding the existing `RequiredClasses` rules (Task 2 Step 5-6); risk 6 (item selection / manifest) is Task 1 Steps 3 and 9.

**Placeholders:** none; the two measured choices (clip picks, icon names) have a rule and a test that catches a wrong value.

**Type consistency:** `Cast::Spell(u32)` (abilities) vs `ActionState::Cast(Cast)` (combat) are different types in different modules; `CastSpec { duration, release, recover }` is built from `cast::Timing` in `spells::plan`; `begin_cast(spell, spec, target)` is called with three arguments everywhere; `Refusal::NotEnough(Power)` replaces `NotEnoughRage` in Task 2 and later tasks use only the new name; `Slowed { left, factor }` is introduced in Task 5 and no earlier task constructs it.
