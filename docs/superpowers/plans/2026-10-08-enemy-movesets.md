# Enemy movesets (the kobold from c2060) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **Do not `git commit`**: in this repo the user commits (nothing is committed yet); every "Checkpoint" step is a gate run, not a commit.

**Goal:** The kobold fights with Dark Souls' own moves: the Blighttown ghoul's (`c2060`) six melee moves retargeted onto the kobold's rig and timed by their TAE windows, damage and poise from the install's params, the ghoul's flinch and death clips as reactions, and a hand-written pattern table choosing the move.

**Architecture:** The extractor (`dsr-cache`, GPL) learns to read a creature chr (`c2060`), retargets its clips onto `Creature\Kobold\Kobold.m2` through a new bone table, and writes `cache/dsr/kobold/*.wscl` plus `monster` manifest lines. The game reads only `cache/dsr/`: a new `monster.rs` loads the kit, a kobold pose system overlays clips on the kobold's `RigPose` (the player's `dsclip` blend code, reused), `npc.rs` plays attacks from the clips (windows, root motion, a hand-capsule hit volume, params damage), reacts from the ghoul's flinch/death clips with a poise number, and `pattern.rs` (pure) picks moves. Without a cache the old kobold stays.

**Tech Stack:** Rust (workspace `C:\Spel\wow-souls`, Bevy 0.18.1 via benilla, glam 0.31), the existing `dsr-formats`/`wscl` crates. Spec: `docs/superpowers/specs/2026-10-08-enemy-movesets-design.md`.

**Conventions for every task:** work in `C:\Spel\wow-souls`; always `-j 4`; install env for tests that read the installs:
`export DSR_DIR="C:/Spel/Dark Souls Remastered" WOW_DATA="C:/Spel/World of Warcraft 1.12/Data"`. Never write inside either install. `cache/` is gitignored. Gates at the end of each task: `cargo fmt --all` then `cargo clippy -j 4 --workspace --all-targets -- -D warnings` then the task's tests.

## File structure

| File | Change |
|---|---|
| `crates/dsr-cache/src/bonemap.rs` | `DsNames` (landmark bone names per DS skeleton), `Rig.names`, the ghoul→kobold pair table and rig |
| `crates/dsr-cache/src/retarget.rs` | uses `rig.names` instead of the c0000 constants |
| `crates/dsr-cache/src/dsr.rs` | `Player.chr`, `load_chr`, creature event lookup |
| `crates/dsr-cache/src/wow.rs` | `skeleton(model)`, `kobold()` |
| `crates/dsr-cache/src/monster.rs` | NEW: the kit record, reaction push measurement, death check, blade-axis measurement |
| `crates/dsr-cache/src/params.rs` | NPC lookups (`npc`, `npc_attack`, `think`), `Behavior.level/poise` |
| `crates/dsr-cache/src/cache.rs` | `extract_monster`, manifest v3, `move` lines gain `level`/`poise` |
| `crates/wscl/src/lib.rs` | `EXTRACTOR_VERSION` 4 |
| `crates/wow-souls/src/combat/action.rs` | `SwingSpec.level/poise` |
| `crates/wow-souls/src/dsclip.rs` | cache v3, `swing_spec(.., level, poise)`, `checked_manifest` split out |
| `crates/wow-souls/src/hits.rs` | `Hit.level/poise/from` |
| `crates/wow-souls/src/monster.rs` | NEW: manifest `monster` lines, `MonsterClips`, `MonsterPlay`, kobold pose system, `AttackSpec` |
| `crates/wow-souls/src/pattern.rs` | NEW: pure pattern table |
| `crates/wow-souls/src/foehit.rs` | NEW: pure hand-capsule sweep |
| `crates/wow-souls/src/npc.rs` | attacks from clips, strike system, reactions, poise |
| `crates/wow-souls/src/dsview.rs` | `WOWSOULS_DSVIEW=kobold:<clip>` |
| `crates/wow-souls/src/main.rs` | `mod monster; mod pattern; mod foehit;` |
| `docs/HANDOVER.md`, spec | status, the two spec corrections |

---

## Phase 1: the extractor

### Task 1: DS landmark names move into the rig (refactor, no behaviour change)

`retarget.rs` and `cache.rs` name c0000's bones (`Pelvis`, `L_Foot`, ...) in constants; an enemy skeleton says `L Foot`. Put them in a `DsNames` the rig carries.

**Files:** Modify `crates/dsr-cache/src/bonemap.rs`, `retarget.rs`, `cache.rs`.

- [ ] **Step 1: Edit `bonemap.rs`.** Replace the block from `/// The DS bone whose rotation turns the hips.` through `pub const DS_FEET ...` and the later `DS_SOLES`, `DS_HAND`, `DS_WEAPON` constants. Delete these constants: `DS_PELVIS`, `DS_LEFT`, `DS_RIGHT`, `DS_FEET`, `DS_SOLES`, `DS_HAND`, `DS_WEAPON` (keep `DS_BLADE_AXIS` and its comment). Add after the `Rig`-independent items (before `pub struct Rig`):

```rust
/// The names of a DS skeleton's landmark bones: they differ between `c0000` (`L_Foot`) and an
/// enemy (`L Foot`).
pub struct DsNames {
    /// The bone whose rotation turns the hips.
    pub pelvis: &'static str,
    /// The shoulders that define the DS left and right.
    pub left: &'static str,
    pub right: &'static str,
    /// Feet → toes pairs that define the DS facing.
    pub feet: [(&'static str, &'static str); 2],
    /// The bones whose lowest one stands on the ground (the counterpart of `Checks::wow_soles`).
    pub soles: &'static [&'static str],
    /// The sword hand and the weapon bone that sits unrotated in it.
    pub hand: &'static str,
    pub weapon: &'static str,
}

/// The player's skeleton.
pub const C0000: DsNames = DsNames {
    pelvis: "Pelvis",
    left: "L_UpperArm",
    right: "R_UpperArm",
    feet: [("L_Foot", "L_Toe0"), ("R_Foot", "R_Toe0")],
    soles: &["L_Foot", "R_Foot", "L_Toe0", "R_Toe0"],
    hand: "R_Hand",
    weapon: "R_Weapon",
};
```

and change `Rig`:

```rust
pub struct Rig<'a> {
    pub pairs: &'a [Pair],
    pub hips: Hips,
    pub legs: Legs,
    pub checks: Checks,
    pub names: &'a DsNames,
}
```
and `HUMAN_MALE_RIG` gets `names: &C0000,` as its last field.

- [ ] **Step 2: Edit `retarget.rs`.**
  - `ds_forward(ds: &Skeleton, up: Vec3)` → `ds_forward(ds: &Skeleton, up: Vec3, feet: &[(&str, &str); 2])` and `for (foot, toe) in bonemap::DS_FEET {` → `for &(foot, toe) in feet {`.
  - `basis_for(ds, clip, wow, legs: &Legs)` → `basis_for(ds: &Skeleton, clip: &DsClip, wow: &WowSkel, rig: &Rig)`; inside: `bone(bonemap::DS_LEFT)` → `bone(rig.names.left)`, `bone(bonemap::DS_RIGHT)` → `bone(rig.names.right)`, `ds_forward(ds, v3(clip.up))` → `ds_forward(ds, v3(clip.up), &rig.names.feet)`, `for [hip, knee, ankle] in legs.ds` → `rig.legs.ds`, `for [hip, knee, ankle] in legs.wow` → `rig.legs.wow`. Remove the now unused `Legs` import only if clippy says so (the `Rig`/`Pair` imports stay).
  - In `retarget`: `let basis = basis_for(ds, clip, wow, &rig.legs)?;` → `basis_for(ds, clip, wow, rig)?`; `bonemap::DS_SOLES.iter().map(|n| ds_bone(n))` → `rig.names.soles.iter().map(|n| ds_bone(n))`; `ds.bone(bonemap::DS_PELVIS).context(...)` → `ds.bone(rig.names.pelvis)`.
  - Tests: `test_rig` gets `names: &bonemap::C0000,` (last field); `the_facing_comes_from_the_feet`: `ds_forward(&feet, Vec3::Y)` → `ds_forward(&feet, Vec3::Y, &bonemap::C0000.feet)` (both calls); install test: `basis_for(&player.skeleton, &clip, &wow, &rig.legs)` → `basis_for(&player.skeleton, &clip, &wow, rig)`, `ds_forward(&player.skeleton, v3(clip.up))` → `ds_forward(&player.skeleton, v3(clip.up), &rig.names.feet)`, `bonemap::DS_SOLES` → `bonemap::C0000.soles`.

- [ ] **Step 3: Edit `cache.rs`.** `extract_clip`: `retarget::basis_for(&player.skeleton, &clip, skel, &rig.legs)?` → `retarget::basis_for(&player.skeleton, &clip, skel, rig)?`. `grip_axis`: `bonemap::DS_WEAPON` → `rig.names.weapon`, `bonemap::DS_HAND` → `rig.names.hand`. Install test `install_the_moves_match_the_data_table`: `player.skeleton.bone(bonemap::DS_HAND)` → `bone(bonemap::C0000.hand)`, `bone(bonemap::DS_WEAPON)` → `bone(bonemap::C0000.weapon)`.

- [ ] **Step 4: Run.** `cargo fmt --all && cargo clippy -j 4 -p dsr-cache --all-targets -- -D warnings && cargo test -j 4 -p dsr-cache` with the install env set. Expected: clippy clean, all dsr-cache tests pass exactly as before (17 tests: `cargo test` summary `test result: ok. 17 passed`).

- [ ] **Step 5: Checkpoint.** Same tests green; `git diff --stat` shows only the three files.

### Task 2: read a creature chr

**Files:** Modify `crates/dsr-cache/src/dsr.rs`.

- [ ] **Step 1: Failing test.** Append to the `tests` module of `dsr.rs`:

```rust
    #[test]
    fn install_the_ghoul_loads_and_its_moves_read_as_measured() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let g = load_chr(Path::new(&dir), "c2060").unwrap();
        assert_eq!(g.skeleton.parents.len(), 53);
        assert!(g.skeleton.bone("L Thigh").is_some() && g.skeleton.bone("R_Weapon").is_some());
        // 135 animations; two of the files are spelled `.HKX` in the archive.
        assert_eq!(g.hkx.len(), 135);
        assert!(g.hkx.contains_key("a00_3301"));
        let h = g.hits("a00_3000");
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].judge, 100);
        assert!((h[0].start - 0.767).abs() < 0.002 && (h[0].end - 0.933).abs() < 0.002);
        // The spear kit lives at +10000: file a01_3000 is animation 13000.
        assert_eq!(g.hits("a01_3000")[0].judge, 180);
        let clip = decode(&g, "a00_3000").unwrap();
        assert_eq!(clip.frames.len(), 87);
        assert!((clip.fps - 30.0).abs() < 0.5);
        assert!((clip.root.last().unwrap()[2] + 1.59).abs() < 0.02);
    }
```

- [ ] **Step 2: Run, expect FAIL** (`load_chr` not found): `cargo test -j 4 -p dsr-cache install_the_ghoul` → `error[E0425]: cannot find function load_chr`.

- [ ] **Step 3: Implement.** In `dsr.rs`: add the field to `Player`:

```rust
    /// A creature's chr id (`c2060`): its animations are `aXX_YYYY.hkx` with the TAE id
    /// `XX × 10000 + YYYY`, all in the one TAE named after the chr. `None` for the player.
    pub chr: Option<String>,
```
set `chr: None` in `load`'s `Ok(Player { .. })`, and replace `events`:

```rust
    /// The TAE events of animation file `a00_6000` (TAE `a00`, animation id 6000); for a creature,
    /// of file `aXX_YYYY` (the chr's TAE, animation id `XX × 10000 + YYYY`).
    pub fn events(&self, stem: &str) -> &[tae::Event] {
        let Some((set, idx)) = stem.split_once('_') else {
            return &[];
        };
        let Ok(idx) = idx.parse::<u32>() else {
            return &[];
        };
        let (key, id) = match &self.chr {
            Some(chr) => {
                let Ok(set) = set.trim_start_matches('a').parse::<u32>() else {
                    return &[];
                };
                (chr.as_str(), set * 10_000 + idx)
            }
            None => (set, idx),
        };
        let Some(anims) = self.taes.get(key) else {
            return &[];
        };
        anims
            .iter()
            .find(|a| a.id == id)
            .map_or(&[][..], |a| a.events.as_slice())
    }
```
and add after `load`:

```rust
/// A creature's data: `chr/<chr>.anibnd.dcx` holds its skeleton, its one TAE and its animation
/// files. File names compare without case (`skeleton.hkx` and `Skeleton.HKX` both occur).
pub fn load_chr(dsr: &Path, chr: &str) -> Result<Player> {
    let mut skeleton = None;
    let mut taes = BTreeMap::new();
    let mut hkx = BTreeMap::new();
    for e in archive(&dsr.join("chr").join(format!("{chr}.anibnd.dcx")))? {
        let name = e.file_name().to_ascii_lowercase();
        let stem = e.stem().to_ascii_lowercase();
        if name == "skeleton.hkx" {
            skeleton = Some(hkx::skeleton::read(&Tagfile::parse(e.data)?)?);
        } else if name.ends_with(".tae") {
            let anims = tae::read(&e.data).with_context(|| format!("TAE {}", e.name))?;
            taes.insert(chr.to_string(), anims);
        } else if name.ends_with(".hkx") {
            hkx.insert(stem, e.data);
        }
    }
    Ok(Player {
        skeleton: skeleton.with_context(|| format!("{chr}.anibnd has no skeleton"))?,
        taes,
        hkx,
        chr: Some(chr.to_string()),
    })
}
```

- [ ] **Step 4: Run.** `cargo test -j 4 -p dsr-cache install_the_ghoul` → PASS; then the whole crate `cargo test -j 4 -p dsr-cache` still passes.

- [ ] **Step 5: Checkpoint.** fmt + clippy clean.

### Task 3: the kobold's skeleton and the ghoul→kobold bone table (the first big risk)

**Files:** Modify `crates/dsr-cache/src/wow.rs`, `bonemap.rs`, `retarget.rs` (test only).

- [ ] **Step 1: `wow.rs`.** Replace `human_male` by a generic reader:

```rust
pub const KOBOLD_M2: &str = "Creature\\Kobold\\Kobold.m2";

/// The bones of the M2 `model` (parents, pivots in Bevy axes) from the install's data directory.
pub fn skeleton(data_dir: &Path, model: &str) -> Result<WowSkel> {
    let mut chain = benilla_formats::Chain::open(data_dir)
        .with_context(|| format!("opening WoW data {}", data_dir.display()))?;
    let bytes = chain.read_file(model)?;
    let fmt = benilla_m2::parse_m2(&mut Cursor::new(&bytes[..]))
        .map_err(|e| anyhow!("parsing {model}: {e:?}"))?;
    let bones = &fmt.model().bones;
    Ok(WowSkel {
        parents: bones.iter().map(|b| b.parent).collect(),
        pivots: bones
            .iter()
            .map(|b| wow_to_bevy(b.pivot.x, b.pivot.y, b.pivot.z))
            .collect(),
    })
}

pub fn human_male(data_dir: &Path) -> Result<WowSkel> {
    skeleton(data_dir, HUMAN_MALE_M2)
}

pub fn kobold(data_dir: &Path) -> Result<WowSkel> {
    skeleton(data_dir, KOBOLD_M2)
}
```
Add a test:

```rust
    #[test]
    fn install_the_kobold_has_its_measured_bones() {
        let Ok(data) = std::env::var("WOW_DATA") else {
            eprintln!("skipped: WOW_DATA not set");
            return;
        };
        let k = kobold(Path::new(&data)).unwrap();
        assert_eq!(k.parents.len(), 72);
        // Key bones 26 root, 4 spine, 6 head; the thighs hang off the hip root 18.
        assert_eq!((k.parents[14], k.parents[27], k.parents[28]), (13, 18, 18));
        assert_eq!((k.parents[35], k.parents[39], k.parents[47]), (29, 27, 39));
        // Left is +Y in WoW, so Bevy -X.
        assert!(k.pivots[27].x < 0.0 && k.pivots[28].x > 0.0, "{} {}", k.pivots[27], k.pivots[28]);
        // Feet stand near the ground, the hips at about 0.74.
        assert!(k.pivots[55].y < 0.05 && (k.pivots[27].y - 0.743).abs() < 0.02);
    }
```

- [ ] **Step 2: `bonemap.rs`: the ghoul's names and the kobold's table.** Append (after `HUMAN_MALE_RIG`):

```rust
/// The ghoul's (`c2060`) skeleton: its bones have spaces in their names.
pub const GHOUL: DsNames = DsNames {
    pelvis: "Pelvis",
    left: "L UpperArm",
    right: "R UpperArm",
    feet: [("L Foot", "L Toe0"), ("R Foot", "R Toe0")],
    soles: &["L Foot", "R Foot", "L Toe0", "R Toe0"],
    hand: "R Hand",
    weapon: "R_Weapon",
};

/// The ghoul → kobold pairs. Kobold indices from `benilla-extract m2bones Creature\Kobold\Kobold.m2`
/// (2026-10-08): 13 root (key bone 26), 14 spine base (4), 15 waist (5), 16 chest, 29 neck,
/// 35 head (6), 18 the hip root under the waist; shoulders 30/31, upper arms 36/37, forearms
/// 44/45, hands 52/53 (left is WoW +Y: the even index), fingers 58/60; thighs 27/28, knees 39/40,
/// ankles 47/48, toes 55/56.
pub const KOBOLD: &[Pair] = &[
    p("Pelvis", "L Thigh", 15, 27, false, false),
    p("Spine", "Spine1", 14, 16, true, false),
    p("Spine1", "Neck", 16, 29, true, false),
    p("Neck", "Head", 29, 35, true, false),
    p("Head", "Neck", 35, 29, false, false),
    p("L Clavicle", "L UpperArm", 30, 36, true, true),
    p("L UpperArm", "L Forearm", 36, 44, true, true),
    p("L Forearm", "L Hand", 44, 52, true, true),
    p("L Hand", "L Finger1", 52, 58, true, true),
    p("R Clavicle", "R UpperArm", 31, 37, true, true),
    p("R UpperArm", "R Forearm", 37, 45, true, true),
    p("R Forearm", "R Hand", 45, 53, true, true),
    p("R Hand", "R Finger1", 53, 60, true, true),
    p("L Thigh", "L Calf", 27, 39, true, true),
    p("L Calf", "L Foot", 39, 47, true, true),
    p("L Foot", "L Toe0", 47, 55, true, false),
    p("R Thigh", "R Calf", 28, 40, true, true),
    p("R Calf", "R Foot", 40, 48, true, true),
    p("R Foot", "R Toe0", 48, 56, true, false),
];

pub const KOBOLD_RIG: Rig<'static> = Rig {
    pairs: KOBOLD,
    hips: Hips {
        wow_hip_root: 18,
        wow_hip_joints: [27, 28],
        ds_hip_joints: ["L Thigh", "R Thigh"],
    },
    legs: Legs {
        ds: [
            ["L Thigh", "L Calf", "L Foot"],
            ["R Thigh", "R Calf", "R Foot"],
        ],
        wow: [[27, 39, 47], [28, 40, 48]],
    },
    checks: Checks {
        cross: &[("L Thigh", "R Thigh", 27, 28), ("Pelvis", "Spine", 15, 14)],
        wow_soles: &[47, 48, 55, 56],
    },
    names: &GHOUL,
};
```

- [ ] **Step 3: Install test that is the real gate.** In `retarget.rs` tests add:

```rust
    /// The ghoul's moves, flinches, death and idle retarget onto the kobold inside the same
    /// gates the player's clips pass; prints the numbers.
    #[test]
    fn install_the_ghoul_retargets_onto_the_kobold() {
        let (Ok(dsr_dir), Ok(wow_data)) = (std::env::var("DSR_DIR"), std::env::var("WOW_DATA"))
        else {
            eprintln!("skipped: DSR_DIR and WOW_DATA not both set");
            return;
        };
        let ghoul = crate::dsr::load_chr(std::path::Path::new(&dsr_dir), "c2060").unwrap();
        let kobold = crate::wow::kobold(std::path::Path::new(&wow_data)).unwrap();
        let rig = &bonemap::KOBOLD_RIG;
        for stem in [
            "a00_0000", "a00_3000", "a00_3001", "a00_3002", "a00_3003", "a00_3004", "a00_3006",
            "a00_2000", "a00_2001", "a00_2004", "a00_2005", "a00_2006", "a00_2007", "a00_9000",
        ] {
            let clip = crate::dsr::decode(&ghoul, stem).unwrap();
            let basis = basis_for(&ghoul.skeleton, &clip, &kobold, rig).unwrap();
            let r = retarget(&ghoul.skeleton, &clip, &kobold, rig, ghoul.flags(stem)).unwrap();
            println!(
                "{stem}: scale {:.4}, direction drift {:.2} deg, cross drift {:.2} deg, \
                 foot gap {:.3} yd on {} contact frames ({:.3} over all)",
                basis.scale,
                r.max_error_deg,
                r.max_cross_deg,
                r.max_foot_gap,
                r.contact_frames,
                r.max_foot_gap_all
            );
            assert!(r.max_error_deg <= crate::cache::MAX_ERROR_DEG, "{stem}: {}", r.max_error_deg);
            assert!(r.max_cross_deg <= crate::cache::MAX_CROSS_DEG, "{stem}: {}", r.max_cross_deg);
            assert!(r.max_foot_gap <= crate::cache::MAX_FOOT_GAP, "{stem}: {}", r.max_foot_gap);
            assert!((0.4..0.9).contains(&basis.scale), "{stem}: scale {}", basis.scale);
            assert!(r.wow.frames.iter().all(|f| f.len() == 72));
        }
    }
```

- [ ] **Step 4: Run.** `cargo test -j 4 -p dsr-cache install_the_ghoul_retargets -- --nocapture`. Expected PASS with scale ≈ 0.62. **If a gate fails**, the failing line names the clip and the measure: a direction drift names no pair, so print per-pair drift by temporarily copying the per-pair loop of `install_the_roll_retargets_onto_the_human`; the usual fix is one pair's `align` flag or its aim bone (a hand aiming at a finger that is really a thumb: try 59/61 for 58/60), never loosening a limit. Re-run until it passes. Fallback if a gate cannot be met for the idle/hunch frames: drop `check` on the offending torso pair (still retargeted, unchecked) and note it in `HANDOVER.md`.

- [ ] **Step 5: Checkpoint.** fmt, clippy `-D warnings`, `cargo test -j 4 -p dsr-cache` all green.

### Task 4: the kit record and the NPC params

**Files:** Create `crates/dsr-cache/src/monster.rs`; modify `params.rs`, `main.rs` (`mod monster;`).

- [ ] **Step 1: `params.rs`: extend `Behavior` and load the NPC tables.** Add to the structs and `load`:

```rust
pub struct Params {
    weapon: Param,
    behavior: Param,
    attack: Param,
    npc: Param,
    npc_behavior: Param,
    npc_attack: Param,
    think: Param,
}

/// What an attack costs and does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Behavior {
    pub stamina: i64,
    /// The damage as a multiple of the base hit: `AtkParam_Pc.atkPhysCorrection` is a percent.
    pub mult: f32,
    /// `AtkParam_Pc.dmgLevel` and `atkSuperArmor`: the flinch level and the poise damage.
    pub level: i64,
    pub poise: i64,
}

/// An `NpcParam` row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Npc {
    pub hp: i64,
    pub poise: i64,
    pub variation: i64,
    pub think: i64,
}

/// A creature attack: `BehaviorParam` (variation, judge) → `AtkParam_Npc`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NpcAttack {
    pub phys: i64,
    pub poise: i64,
    pub level: i64,
    pub radius: f32,
    pub radius2: f32,
}

/// An `NpcThinkParam` row's distance bands (DS metres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Think {
    pub near: f32,
    pub mid: f32,
    pub far: f32,
    pub out: f32,
}
```
In `load`, after the existing three `take(...)` calls add `npc: take("NpcParam")?, npc_behavior: take("BehaviorParam")?, npc_attack: take("AtkParam_Npc")?, think: take("NpcThinkParam")?`. In `behavior()` extend the returned struct:

```rust
        Ok(Behavior {
            stamina: b.int(row, "stamina")?,
            mult: a.int(arow, "atkPhysCorrection")? as f32 / 100.0,
            level: a.int(arow, "dmgLevel")?,
            poise: a.int(arow, "atkSuperArmor")?,
        })
```
and add in `impl Params`:

```rust
    pub fn npc(&self, id: i32) -> Result<Npc> {
        let n = &self.npc;
        let row = n
            .row_of(id)
            .with_context(|| format!("NpcParam has no row {id}"))?;
        Ok(Npc {
            hp: n.int(row, "hp")?,
            poise: n.int(row, "superArmorDurability")?,
            variation: n.int(row, "behaviorVariationId")?,
            think: n.int(row, "aiThinkId")?,
        })
    }

    /// A creature's attack: the `BehaviorParam` row with this variation and judge (exactly one,
    /// an attack, not a bullet) and the `AtkParam_Npc` row it points at.
    pub fn npc_attack(&self, variation: i64, judge: i32) -> Result<NpcAttack> {
        let b = &self.npc_behavior;
        let mut rows = (0..b.len()).filter(|&r| {
            matches!(
                (b.int(r, "variationId"), b.int(r, "behaviorJudgeId")),
                (Ok(v), Ok(j)) if v == variation && j == i64::from(judge)
            )
        });
        let row = rows.next().with_context(|| {
            format!("BehaviorParam has no row for variation {variation}, judge {judge}")
        })?;
        ensure!(
            rows.next().is_none(),
            "BehaviorParam has two rows for variation {variation}, judge {judge}"
        );
        let id = b.id(row);
        ensure!(
            b.int(row, "refType")? == 0,
            "BehaviorParam row {id} is not an attack"
        );
        let atk = i32::try_from(b.int(row, "refId")?).context("refId")?;
        let a = &self.npc_attack;
        let arow = a
            .row_of(atk)
            .with_context(|| format!("AtkParam_Npc has no row {atk} (BehaviorParam row {id})"))?;
        Ok(NpcAttack {
            phys: a.int(arow, "atkPhys")?,
            poise: a.int(arow, "atkSuperArmor")?,
            level: a.int(arow, "dmgLevel")?,
            radius: a.float(arow, "hit0_Radius")?,
            radius2: a.float(arow, "hit1_Radius")?,
        })
    }

    pub fn think(&self, id: i32) -> Result<Think> {
        let t = &self.think;
        let row = t
            .row_of(id)
            .with_context(|| format!("NpcThinkParam has no row {id}"))?;
        Ok(Think {
            near: t.float(row, "nearDist")?,
            mid: t.float(row, "midDist")?,
            far: t.float(row, "farDist")?,
            out: t.float(row, "outDist")?,
        })
    }
```
Test (append to `params.rs` tests):

```rust
    #[test]
    fn install_the_ghoul_reads_as_measured() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let p = load(Path::new(&dir)).unwrap();
        assert_eq!(
            p.npc(206_000).unwrap(),
            Npc { hp: 203, poise: 0, variation: 20_600, think: 206_000 }
        );
        assert!(p.npc(1).unwrap().hp == 150);
        assert!(p.npc(9_999_999).is_err());
        // (judge, atkPhys, poise damage, level, radius, radius2): the spec's table.
        for (judge, phys, poise, level, r1, r2) in [
            (100, 138, 10, 1, 0.30, 0.0),
            (110, 142, 10, 1, 0.30, 0.0),
            (120, 195, 20, 2, 0.30, 0.0),
            (130, 186, 20, 2, 0.40, 0.0),
            (140, 106, 30, 5, 0.20, 0.20),
            (160, 124, 20, 1, 0.60, 0.60),
        ] {
            let a = p.npc_attack(20_600, judge).unwrap();
            assert_eq!((a.phys, a.poise, a.level), (phys, poise, level), "judge {judge}");
            assert!((a.radius - r1).abs() < 1.0e-6 && (a.radius2 - r2).abs() < 1.0e-6, "judge {judge}: {a:?}");
        }
        assert!(p.npc_attack(20_600, 150).is_err(), "the head throw is a bullet");
        assert!(p.npc_attack(20_600, 12_345).is_err());
        let t = p.think(206_000).unwrap();
        assert_eq!((t.near, t.mid, t.far, t.out), (2.0, 5.0, 10.0, 20.0));
        // The player's moves: R1 is level 1, R2 level 2, no poise damage.
        assert_eq!((p.behavior(2300, 0).unwrap().level, p.behavior(2300, 100).unwrap().level), (1, 2));
        assert_eq!(p.behavior(2300, 0).unwrap().poise, 0);
    }
```
Check `npc(1).hp == 150` is the "pad" row (`あたりさわりのないデータ`, hp 150 measured in the survey); delete that line if the executor finds it noisy.

- [ ] **Step 2: Create `crates/dsr-cache/src/monster.rs`** (the kit record, the checks, the measurements):

```rust
//! A creature's kit: which Dark Souls animation is which move, flinch and death, for one
//! creature of the game. Hand-made, like the bone map and the weapon classes. The ghoul's
//! animation files are `aXX_YYYY` (TAE id `XX × 10000 + YYYY`); the sword kit is `a00_3xxx`.

use anyhow::{ensure, Result};
use glam::Vec3;

use crate::params::Params;
use crate::retarget::Retargeted;

/// One attack: the game's name for it, its clip and the judge id its TAE hit window must carry.
pub struct Attack {
    pub name: &'static str,
    pub clip: &'static str,
    pub judge: i32,
}

/// One flinch clip and the hit level it answers.
pub struct React {
    pub level: u8,
    pub clip: &'static str,
}

pub struct Kit {
    pub name: &'static str,
    pub chr: &'static str,
    /// An `NpcParam` row of the creature, and the behavior variation it must carry.
    pub npc: i32,
    pub variation: i64,
    pub attacks: &'static [Attack],
    pub reacts: &'static [React],
    pub death: &'static str,
}

/// The kobold wears the Blighttown ghoul's sword kit. Left out: the plunge (`a00_3401`, judge
/// 170), the feigned death (`a00_9061`, 240) and the head throw (judge 150, a bullet).
/// Flinches: `a00_2000`-`2002` are 0.6 s head-back clips, `2004`-`2007` 1.0 s ones; which way
/// each pushes the head is measured ([`push`]), not assumed. Death: `a00_9000`, `9002` and `9003`
/// end in the same slumped pose.
pub const KOBOLD: Kit = Kit {
    name: "kobold",
    chr: "c2060",
    npc: 206_000,
    variation: 20_600,
    attacks: &[
        Attack { name: "small_1", clip: "a00_3000", judge: 100 },
        Attack { name: "small_2", clip: "a00_3001", judge: 110 },
        Attack { name: "big", clip: "a00_3002", judge: 120 },
        Attack { name: "run_strike", clip: "a00_3003", judge: 130 },
        Attack { name: "body_check", clip: "a00_3004", judge: 140 },
        Attack { name: "pounce", clip: "a00_3006", judge: 160 },
    ],
    reacts: &[
        React { level: 1, clip: "a00_2000" },
        React { level: 1, clip: "a00_2001" },
        React { level: 1, clip: "a00_2004" },
        React { level: 1, clip: "a00_2005" },
        React { level: 1, clip: "a00_2006" },
        React { level: 2, clip: "a00_2007" },
        React { level: 2, clip: "a00_2004" },
        React { level: 2, clip: "a00_2005" },
        React { level: 2, clip: "a00_2006" },
    ],
    death: "a00_9000",
};

impl Kit {
    /// The `NpcParam` row says what this record says, and every attack's judge resolves, so a
    /// kit recorded wrong cannot pass.
    pub fn check(&self, params: &Params) -> Result<()> {
        let n = params.npc(self.npc)?;
        ensure!(
            n.variation == self.variation,
            "NpcParam {} has behavior variation {}; the {} record says {}",
            self.npc,
            n.variation,
            self.name,
            self.variation
        );
        for a in self.attacks {
            params
                .npc_attack(self.variation, a.judge)
                .map_err(|e| e.context(format!("attack {} ({})", a.name, a.clip)))?;
        }
        Ok(())
    }

    /// Every distinct clip of the kit, in file order of the record.
    pub fn clips(&self) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = Vec::new();
        for c in self
            .attacks
            .iter()
            .map(|a| a.clip)
            .chain(self.reacts.iter().map(|r| r.clip))
            .chain(std::iter::once(self.death))
        {
            if !v.contains(&c) {
                v.push(c);
            }
        }
        v
    }
}

/// Which way a flinch clip pushes the head: the head's largest deviation from its first frame,
/// relative to the pelvis, over the clip, as `(forward, left)` in the creature's own frame (the
/// retargeted DS clip is in Bevy axes, forward -Z, left -X). A hit from the front pushes the head
/// back: forward negative.
pub fn push(r: &Retargeted, head: usize, pelvis: usize) -> Result<(f32, f32)> {
    let rel = |f: usize| {
        let fr = &r.ds.frames[f];
        Vec3::from_array(fr[head].t) - Vec3::from_array(fr[pelvis].t)
    };
    ensure!(!r.ds.frames.is_empty(), "{} has no frames", r.ds.name);
    let base = rel(0);
    let mut best = (0.0_f32, Vec3::ZERO);
    for f in 0..r.ds.frames.len() {
        let d = rel(f) - base;
        let flat = Vec3::new(d.x, 0.0, d.z).length();
        if flat > best.0 {
            best = (flat, d);
        }
    }
    Ok((-best.1.z, -best.1.x))
}

/// The death clip ends lying or slumped and holds still: the pelvis ends lower than 0.6 of its
/// first height and moves less than 0.02 yd over the last ten frames.
pub fn check_death(r: &Retargeted, pelvis: usize) -> Result<()> {
    let f = &r.ds.frames;
    ensure!(f.len() > 12, "{}: {} frames is no death", r.ds.name, f.len());
    let y = |i: usize| f[i][pelvis].t[1];
    let p = |i: usize| Vec3::from_array(f[i][pelvis].t);
    let last = f.len() - 1;
    ensure!(
        y(last) < 0.6 * y(0),
        "{}: the pelvis ends at {:.2} yd from {:.2}: not down",
        r.ds.name,
        y(last),
        y(0)
    );
    ensure!(
        (last - 10..=last).all(|i| (p(i) - p(last)).length() < 0.02),
        "{}: the pelvis is still moving at the end",
        r.ds.name
    );
    Ok(())
}

/// The weapon bone's axis that reaches furthest ahead of the creature over the hit windows of
/// `clips` (each: the DS model-space clip and its hit windows): the six unit axes tried, the
/// best returned with its mean forward reach. DS forward is Bevy -Z in the retargeted clips.
pub fn blade_axis(clips: &[(&wscl::Clip, usize)]) -> (Vec3, f32) {
    let axes = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z];
    let mut best = (Vec3::Y, f32::NEG_INFINITY);
    for axis in axes {
        let (mut sum, mut n) = (0.0_f32, 0_u32);
        for &(clip, weapon) in clips {
            for h in &clip.hits {
                let from = (h.start * clip.fps).floor() as usize;
                let to = ((h.end * clip.fps).ceil() as usize).min(clip.frames.len() - 1);
                for f in from..=to {
                    let t = &clip.frames[f][weapon];
                    let hand = Vec3::from_array(t.t);
                    let dir = glam::Quat::from_array(t.r).normalize() * axis;
                    sum += -(hand + dir * 0.5).z + hand.z; // forward reach of a half-unit blade
                    n += 1;
                }
            }
        }
        let mean = sum / n.max(1) as f32;
        if mean > best.1 {
            best = (axis, mean);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kobold_kit_names_distinct_attacks_and_clips() {
        let k = &KOBOLD;
        let mut names: Vec<_> = k.attacks.iter().map(|a| a.name).collect();
        let mut clips: Vec<_> = k.attacks.iter().map(|a| a.clip).collect();
        names.sort_unstable();
        names.dedup();
        clips.sort_unstable();
        clips.dedup();
        assert_eq!((names.len(), clips.len()), (6, 6));
        assert!(k.attacks.iter().all(|a| a.clip.starts_with("a00_3")));
        // Flinch clips repeat across levels; the kit lists each file once.
        assert_eq!(k.clips().len(), 6 + 5 + 1);
    }

    #[test]
    fn install_the_kit_record_agrees_with_the_params() {
        let Ok(dir) = std::env::var("DSR_DIR") else {
            eprintln!("DSR_DIR not set; skipped");
            return;
        };
        let p = crate::params::load(std::path::Path::new(&dir)).unwrap();
        KOBOLD.check(&p).unwrap();
        let wrong = Kit { variation: 20_601, ..KOBOLD };
        assert!(wrong.check(&p).unwrap_err().to_string().contains("20601"));
    }

    fn clip_with(frames: Vec<Vec<wscl::Trs>>) -> Retargeted {
        let c = wscl::Clip {
            name: "t".into(),
            fps: 30.0,
            model_space: true,
            parents: vec![-1, 0],
            driven: vec![true; 2],
            frames,
            root_motion: Vec::new(),
            flags: Vec::new(),
            hits: Vec::new(),
        };
        Retargeted {
            wow: c.clone(),
            ds: c,
            max_error_deg: 0.0,
            max_cross_deg: 0.0,
            max_foot_gap: 0.0,
            max_foot_gap_all: 0.0,
            contact_frames: 0,
        }
    }

    fn at(pelvis: [f32; 3], head: [f32; 3]) -> Vec<wscl::Trs> {
        [pelvis, head]
            .map(|t| wscl::Trs { t, ..wscl::Trs::IDENTITY })
            .to_vec()
    }

    #[test]
    fn a_head_thrown_back_pushes_backward_and_a_head_to_the_left_pushes_left() {
        // Bevy: forward -Z, left -X. The head ends 0.3 behind (+Z) and 0.1 to the left (-X).
        let r = clip_with(vec![at([0.0; 3], [0.0, 1.0, 0.0]), at([0.0; 3], [-0.1, 1.0, 0.3])]);
        let (fwd, left) = push(&r, 1, 0).unwrap();
        assert!((fwd + 0.3).abs() < 1.0e-6 && (left - 0.1).abs() < 1.0e-6, "{fwd} {left}");
    }

    #[test]
    fn a_death_must_end_low_and_still() {
        let mut frames: Vec<_> = (0..20)
            .map(|i| at([0.0, 1.0 - 0.05 * i as f32, 0.0], [0.0, 1.5, 0.0]))
            .collect();
        assert!(check_death(&clip_with(frames.clone()), 0).is_err(), "ends at 0.05 · 19 = 0.05 yd: low, but still falling");
        frames.extend((0..12).map(|_| at([0.0, 0.05, 0.0], [0.0, 0.2, 0.0])));
        assert!(check_death(&clip_with(frames), 0).is_ok());
        let standing = vec![at([0.0, 1.0, 0.0], [0.0, 1.5, 0.0]); 20];
        assert!(check_death(&clip_with(standing), 0).is_err());
    }
}
```
(Note on the first assertion: the first 20 frames end at y = 0.05 but the last ten frames move 0.05/frame → `still` fails: `is_err`.)

Add `mod monster;` to `main.rs` (after `mod moveset;`).

- [ ] **Step 3: Run.** `cargo test -j 4 -p dsr-cache` (install env). Expected: the new unit tests and `install_the_kit_record_agrees_with_the_params`, `install_the_ghoul_reads_as_measured` PASS. If `clippy` complains about `dead_code` for items not yet used (`Kit::clips`, `push`, `check_death`, `blade_axis`), they are used in Task 5; run clippy after Task 5, or add `#![allow(dead_code)]` temporarily at the top of `monster.rs` and remove it in Task 5.

- [ ] **Step 4: Measure the ghoul's blade axis and pin it.** Add an install test in `monster.rs` tests:

```rust
    /// The ghoul's `R_Weapon` axis that leads the three sword moves is the one the extractor writes
    /// as the reach (`GHOUL_BLADE_AXIS`); this measures it again on the install.
    #[test]
    fn install_the_ghoul_blade_axis_is_the_measured_one() {
        let (Ok(dsr_dir), Ok(wow_data)) = (std::env::var("DSR_DIR"), std::env::var("WOW_DATA"))
        else {
            eprintln!("skipped: DSR_DIR and WOW_DATA not both set");
            return;
        };
        let ghoul = crate::dsr::load_chr(std::path::Path::new(&dsr_dir), "c2060").unwrap();
        let kobold = crate::wow::kobold(std::path::Path::new(&wow_data)).unwrap();
        let rig = &crate::bonemap::KOBOLD_RIG;
        let weapon = ghoul.skeleton.bone("R_Weapon").unwrap();
        let mut keep = Vec::new();
        for clip in ["a00_3000", "a00_3001", "a00_3002"] {
            let d = crate::dsr::decode(&ghoul, clip).unwrap();
            let mut r = crate::retarget::retarget(&ghoul.skeleton, &d, &kobold, rig, ghoul.flags(clip)).unwrap();
            r.ds.hits = ghoul.hits(clip);
            keep.push(r.ds);
        }
        let refs: Vec<_> = keep.iter().map(|c| (c, weapon)).collect();
        let (axis, reach) = blade_axis(&refs);
        println!("ghoul blade axis {axis} (mean forward reach {reach:.3})");
        assert_eq!(axis, GHOUL_BLADE_AXIS);
    }
```
and add the constant near the top of `monster.rs`: `pub const GHOUL_BLADE_AXIS: Vec3 = Vec3::Y;`. Run `cargo test -j 4 -p dsr-cache install_the_ghoul_blade_axis -- --nocapture`; if the printed axis is not `Y`, change `GHOUL_BLADE_AXIS` to the printed one (this constant is measured, as `DS_BLADE_AXIS` was) and re-run to PASS.

- [ ] **Step 5: Checkpoint.** fmt, clippy, `cargo test -j 4 -p dsr-cache` green.

### Task 5: write the kobold's cache

**Files:** Modify `crates/dsr-cache/src/cache.rs`, `crates/wscl/src/lib.rs`, `crates/dsr-cache/src/main.rs` (doc only).

- [ ] **Step 1: Versions.** `wscl/src/lib.rs`: `pub const EXTRACTOR_VERSION: u32 = 4;`. `cache.rs`: `pub const MANIFEST_VERSION: u32 = 3;`.

- [ ] **Step 2: Generalise the helpers.** In `cache.rs`:
  - `write_pair(out, name, r)` → `write_pair(out: &Path, wow_dir: &str, ds_dir: &str, name: &str, r: &Retargeted)` writing `out.join(wow_dir)` / `out.join(ds_dir)`; its two existing calls pass `"human_male", "c0000"`.
  - `extract_clip(player, skel, name)` → `extract_clip(player: &Player, skel: &WowSkel, rig: &Rig, name: &str)` with `let clip = dsr::decode(player, name)?; let mut r = retarget::retarget(&player.skeleton, &clip, skel, rig, player.flags(name))?;` and `basis_for(.., rig)`; the existing three calls pass `&bonemap::HUMAN_MALE_RIG`. In `grip_axis` add a parameter `blade: Vec3` replacing `bonemap::DS_BLADE_AXIS` (the three call sites pass `bonemap::DS_BLADE_AXIS`).
  - The player's `move` line: `writeln!(move_lines, "move {} {} judge {judge} stamina {} mult {:.2} level {} poise {}", m.name, m.clip, b.stamina, b.mult, b.level, b.poise)?;`.
  - `extract` takes the monster part at its end: change the signature unchanged, but before `let mut m = String::new();` call `let monster_lines = extract_monster(dsr_dir, wow_dir, out, &params, &mut worst)?;` and after the `grip` line `m.push_str(&monster_lines);`. Add `source_line(&dsr_dir.join("chr").join("c2060.anibnd.dcx"))` next to the c0000 one.

- [ ] **Step 3: `extract_monster`.** Add to `cache.rs`:

```rust
/// The kobold's kit: every clip retargeted onto the kobold, written to `kobold/` (kobold bones,
/// local) and `c2060/` (ghoul bones, model space); returns the manifest's `monster` lines.
fn extract_monster(
    dsr_dir: &Path,
    wow_dir: &Path,
    out: &Path,
    params: &params::Params,
    worst: &mut Worst,
) -> Result<String> {
    let kit = &monster::KOBOLD;
    kit.check(params)?;
    let ghoul = dsr::load_chr(dsr_dir, kit.chr)?;
    let skel = wow::kobold(wow_dir)?;
    let rig = &bonemap::KOBOLD_RIG;
    let bone = |n: &str| {
        ghoul
            .skeleton
            .bone(n)
            .with_context(|| format!("the ghoul has no bone {n}"))
    };
    let (head, pelvis) = (bone("Head")?, bone(rig.names.pelvis)?);
    let wow_hand = rig
        .pairs
        .iter()
        .find(|p| p.ds == rig.names.hand)
        .map(|p| p.wow)
        .context("the kobold table has no hand")?;
    let npc = params.npc(kit.npc)?;
    let think = params.think(i32::try_from(npc.think)?)?;
    let mut lines = String::new();
    let mut extracted = std::collections::BTreeMap::new();
    for clip in kit.clips() {
        let e = extract_clip(&ghoul, &skel, rig, clip).with_context(|| format!("kit clip {clip}"))?;
        worst.add(&e.r);
        write_pair(out, "kobold", kit.chr, clip, &e.r)?;
        println!("{}", summary(clip, &e.r));
        extracted.insert(clip, e);
    }
    let scale = extracted
        .values()
        .next()
        .map(|e| e.basis.scale)
        .context("the kit has no clips")?;
    writeln!(
        lines,
        "monster {} chr {} npc {} hp {} poise {} scale {scale:.4}",
        kit.name, kit.chr, kit.npc, npc.hp, npc.poise
    )?;
    for a in kit.attacks {
        let e = &extracted[a.clip];
        let judge = e
            .r
            .wow
            .hits
            .first()
            .map(|h| h.judge)
            .with_context(|| format!("{} has no hit window", a.clip))?;
        ensure!(
            judge == a.judge,
            "{}: the TAE judge is {judge}, the kit says {}",
            a.clip,
            a.judge
        );
        let n = params.npc_attack(kit.variation, judge)?;
        writeln!(
            lines,
            "attack {} {} judge {judge} phys {} poise {} level {} radius {:.2} radius2 {:.2}",
            a.name, a.clip, n.phys, n.poise, n.level, n.radius, n.radius2
        )?;
    }
    for r in kit.reacts {
        let e = &extracted[r.clip];
        let (fwd, left) = monster::push(&e.r, head, pelvis)?;
        writeln!(lines, "react {} {} push {fwd:.3} {left:.3}", r.level, r.clip)?;
        println!("  {} (level {}) pushes the head ({fwd:+.2} forward, {left:+.2} left)", r.clip, r.level);
    }
    monster::check_death(&extracted[kit.death].r, pelvis)?;
    writeln!(lines, "death {}", kit.death)?;
    writeln!(
        lines,
        "think near {:.1} mid {:.1} far {:.1} out {:.1}",
        think.near, think.mid, think.far, think.out
    )?;
    // The blade axis in the kobold's right-hand bone frame, as the player's `grip` line does.
    let blade = monster::GHOUL_BLADE_AXIS;
    let first = kit.attacks.first().context("the kit has no attacks")?;
    let hand = grip_axis(&extracted[first.clip], &ghoul.skeleton, rig, blade)
        .with_context(|| first.clip.to_string())?;
    for a in kit.attacks {
        let h = grip_axis(&extracted[a.clip], &ghoul.skeleton, rig, blade)
            .with_context(|| a.clip.to_string())?;
        ensure!(
            hand.angle_between(h).to_degrees() < MAX_GRIP_DRIFT_DEG,
            "{}: its blade axis differs from the first move's",
            a.clip
        );
    }
    writeln!(lines, "reach {wow_hand} {:.5} {:.5} {:.5}", hand.x, hand.y, hand.z)?;
    Ok(lines)
}
```
`Worst` must be `pub(crate)`-visible inside the module (it is in the same file, fine). Add `use crate::monster;` to the imports. `extracted` is a `BTreeMap<&'static str, Extracted>`; `Extracted` has `r` and `basis`.

- [ ] **Step 4: Run the extractor.** `cargo run -j 4 -p dsr-cache --bin wow-souls-dsr -- extract` (default paths). Expected: the player's lines as before plus 12 kit lines (`a00_3000: 87 frames at 30.0 fps (2.867 s) ...`), the `pushes the head` lines, and `cache/dsr/manifest.txt` written with `monster kobold chr c2060 npc 206000 hp 203 poise 0 scale 0.6...`, six `attack` lines (`phys 138 ... 195 ... 106 ... 124`), nine `react` lines, `death a00_9000`, `think near 2.0 mid 5.0 far 10.0 out 20.0`, `reach 53 ...`, and `move` lines with `level 1 poise 0` / `level 2 poise 0`. Check by hand: `cat cache/dsr/manifest.txt`.

- [ ] **Step 5: Install test that pins the kit to the spec's table** (append to `cache.rs` tests):

```rust
    #[test]
    fn install_the_kobolds_moves_match_the_data_table() {
        let (Ok(dsr_dir), Ok(wow_dir)) = (std::env::var("DSR_DIR"), std::env::var("WOW_DATA"))
        else {
            eprintln!("DSR_DIR and WOW_DATA not both set; skipped");
            return;
        };
        let ghoul = dsr::load_chr(Path::new(&dsr_dir), "c2060").unwrap();
        let skel = wow::kobold(Path::new(&wow_dir)).unwrap();
        let rig = &bonemap::KOBOLD_RIG;
        // clip, judge, length s, hit window s, lunge in DS metres (the clip's root motion).
        let table = [
            ("a00_3000", 100, 2.867, (0.767, 0.933), 1.59),
            ("a00_3001", 110, 2.167, (0.833, 1.033), 0.49),
            ("a00_3002", 120, 3.167, (1.433, 1.667), 1.35),
            ("a00_3003", 130, 3.167, (0.767, 0.967), 3.17),
            ("a00_3004", 140, 1.400, (0.600, 0.833), 1.43),
            ("a00_3006", 160, 3.333, (2.067, 2.167), 3.37),
        ];
        for (clip, judge, len, hit, lunge) in table {
            let e = extract_clip(&ghoul, &skel, rig, clip).unwrap();
            let w = &e.r.wow;
            assert!((w.duration() - len).abs() < 0.002, "{clip}: {}", w.duration());
            let h = w.hits.first().unwrap();
            assert_eq!(h.judge, judge, "{clip}");
            assert!((h.start - hit.0).abs() < 0.001 && (h.end - hit.1).abs() < 0.001, "{clip}: {h:?}");
            // The clip's root motion times the leg ratio is what the kobold travels (±2 %).
            let z = -w.root_motion.last().unwrap()[2];
            let want = lunge * e.basis.scale;
            assert!((z - want).abs() < 0.02 * want.max(0.1), "{clip}: lunge {z} vs {want}");
        }
    }
```

- [ ] **Step 6: Checkpoint.** fmt, clippy (remove any temporary `allow(dead_code)`), `cargo test -j 4 -p dsr-cache` all green; the extractor ran and `cache/dsr/kobold/*.wscl` (12 files) and `cache/dsr/c2060/*.wscl` exist.

---

## Phase 2: the game reads the kit and shows the look

### Task 6: `level` and `poise` through the player's moves

The kobold's flinch needs the hit's level and poise damage, which come from the player's move lines.

**Files:** Modify `combat/action.rs`, `dsclip.rs`, `hits.rs`.

- [ ] **Step 1: `SwingSpec`** (action.rs): add two fields after `mult`:

```rust
    /// The flinch level of the hit (`AtkParam_Pc.dmgLevel`) and its poise damage
    /// (`atkSuperArmor`).
    pub level: u8,
    pub poise: f32,
```
`SwingSpec::procedural`: add `level: 1, poise: 0.0,`. In action.rs's test helper (`let s = |duration, hit, chain_from, evade_from, move_from, cost, mult| SwingSpec { .. }`, line ~526) add `level: 1, poise: 0.0,` to the struct literal.

- [ ] **Step 2: dsclip.rs.** `swing_spec(clip, cost, mult)` → `swing_spec(clip: &wscl::Clip, cost: f32, mult: f32, level: u8, poise: f32)` with `level, poise` in the literal; `const CACHE_VERSION: &str = "3";`; `MoveLine` gains `level: u8, poise: f32`; `parse_move`:

```rust
    let [_, name, clip, "judge", _, "stamina", stamina, "mult", mult, "level", level, "poise", poise] =
        w[..]
    else {
        return Err(bad());
    };
    Ok(MoveLine {
        kind: ...same...,
        clip: clip.to_string(),
        stamina: stamina.parse().map_err(|_| bad())?,
        mult: mult.parse().map_err(|_| bad())?,
        level: level.parse().map_err(|_| bad())?,
        poise: poise.parse().map_err(|_| bad())?,
    })
```
and `swing_spec(&wow, line.stamina, line.mult, line.level, line.poise)?` in `load`. Update the tests: `a_move_takes_its_timing_from_its_clip` calls `swing_spec(&c, 20.0, 1.35, 1, 0.0)` (three places: the `unwrap()` and the two `unwrap_err()`s, with `1, 0.0` appended) and also asserts `assert_eq!((s.level, s.poise), (1, 0.0));`; `whole_cache` manifest line becomes `"move {} {clip} judge 0 stamina {} mult 1.{i}0 level {} poise 0\n"` with a fourth argument `if i < 3 { 1 } else { 2 }`; the `parse_move` negative tests get the new tail: `"move r1_1 a23_3000 judge 0 stamina x mult 1.0 level 1 poise 0"` and `"move r9_9 a23_3000 judge 0 stamina 20 mult 1.0 level 1 poise 0"`.

- [ ] **Step 3: hits.rs.** `Hit` gains fields:

```rust
    /// The blow's flinch level and poise damage (the move's), and where it came from.
    pub level: u8,
    pub poise: f32,
    pub from: Vec3,
```
`strike` needs the avatar's position: change its avatar query to `Query<(&Action, &HeldBuilt, &mut BladeTrail, &GlobalTransform), With<Avatar>>` and destructure `let Ok((action, held, mut trail, avatar_at)) = avatar.single_mut()`; in the `hits.write(Hit { .. })` add `level: swing.spec.level, poise: swing.spec.poise, from: avatar_at.translation(),`.

- [ ] **Step 4: Run.** `cargo test -j 4 -p wow-souls` → all existing tests pass (188). Re-extract is already done (Task 5): `cargo run -j 4 -p wow-souls` is not needed yet.

- [ ] **Step 5: Checkpoint.** fmt + clippy clean.

### Task 7: the monster manifest loader

**Files:** Create `crates/wow-souls/src/monster.rs` (first half); modify `dsclip.rs` (split out `checked_manifest`), `main.rs`.

- [ ] **Step 1: `dsclip.rs`: split the manifest check.** The staleness and version logic in `load` is wanted by the monster loader too. Add

```rust
/// The manifest's text once its versions are this game's and every `source` file is as it was
/// at extraction; `Err` says why the cache is unusable.
pub fn checked_manifest(dir: &Path) -> Result<String, String> {
    let manifest = std::fs::read_to_string(dir.join("manifest.txt"))
        .map_err(|e| format!("no manifest in {} ({e})", dir.display()))?;
    let (mut cache_version, mut extractor) = (false, false);
    for line in manifest.lines() {
        let mut w = line.splitn(4, ' ');
        match (w.next(), w.next(), w.next(), w.next()) {
            (Some("wscl-cache"), Some(v), _, _) => {
                if v != CACHE_VERSION {
                    return Err(format!("cache version {v}, expected {CACHE_VERSION}"));
                }
                cache_version = true;
            }
            (Some("extractor"), Some(v), _, _) => {
                if v != wscl::EXTRACTOR_VERSION.to_string() {
                    return Err(format!(
                        "extractor version {v}, expected {}",
                        wscl::EXTRACTOR_VERSION
                    ));
                }
                extractor = true;
            }
            (Some("source"), Some(len), Some(mtime), Some(path)) => {
                let meta = std::fs::metadata(path).map_err(|e| format!("source {path}: {e}"))?;
                let now = meta
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs().to_string());
                if meta.len().to_string() != len || now.as_deref() != Some(mtime) {
                    return Err(format!("stale: {path} changed since extraction"));
                }
            }
            _ => {}
        }
    }
    if !(cache_version && extractor) {
        return Err("manifest lacks a version".into());
    }
    Ok(manifest)
}
```
and make `load` start with `let manifest = checked_manifest(dir)?;` followed by a reduced loop that collects only `roll`, `move`, `grip` lines (delete the version/source arms from it, keep `(Some("roll"), Some(name), _, _)`, `(Some("move"), ..)`, `(Some("grip"), ..)`, `_ => {}`), and remove the now-unused `cache_version`/`extractor` locals and the `if !(cache_version && extractor)` check from `load`. Make `read`'s clip reader reusable: lift the `read` closure to `pub fn read_clip(dir: &Path, sub: &str, name: &str) -> Result<wscl::Clip, String>` (same body) and have `load` call `read_clip(dir, "human_male", ..)`. The manifest tests (`the_manifest_must_carry_this_games_versions`) keep passing because the error strings are unchanged.

- [ ] **Step 2: Failing tests for the monster parser.** Create `crates/wow-souls/src/monster.rs` with this content (the whole first half; the pose system comes in Task 8):

```rust
//! The kobold's Dark Souls kit from `cache/dsr/` (written by `wow-souls-dsr extract`): the
//! ghoul's six melee moves, its flinch clips and its death, retargeted onto the kobold rig, with
//! each move's hit window, damage and poise from the install's data. Loaded at startup with the
//! player's clips; without a cache, or with a stale one, the old kobold fights.

use std::path::Path;

use bevy::prelude::*;

/// What one kobold move is, from its clip's TAE window and the install's params.
#[derive(Debug, Clone, PartialEq)]
pub struct AttackSpec {
    pub name: String,
    /// The clip's length (s).
    pub length: f32,
    /// The hit window (s into the clip).
    pub hit: (f32, f32),
    /// The damage to the player: `atkPhys × DAMAGE_K`, rounded.
    pub damage: f32,
    /// Poise damage and flinch level of the blow.
    pub poise: f32,
    pub level: u8,
    /// The hit volume's radius (DS metres; the game scales it) and the second hand's.
    pub radius: f32,
    pub radius2: f32,
}

/// `damage = atkPhys × DAMAGE_K`: small hit 1 (`atkPhys` 138) stays today's 25.
pub const DAMAGE_K: f32 = 25.0 / 138.0;

/// One move: its clips and its numbers.
pub struct AttackClip {
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
    pub spec: AttackSpec,
}

/// One flinch clip, the level it answers and where it pushes the head.
pub struct ReactClip {
    pub level: u8,
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
    /// `(forward, left)` of the head's push in the creature's frame (negative forward: head back).
    pub push: Vec2,
}

/// The kobold's death clip pair.
pub struct DeathClip {
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
}

/// Where the kobold's blade points: its axis in the right-hand bone's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reach {
    pub bone: u16,
    pub axis: Vec3,
}

/// The distance bands of the ghoul's `NpcThinkParam` (DS metres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Think {
    pub near: f32,
    pub mid: f32,
    pub far: f32,
    pub out: f32,
}

/// The loaded kit.
#[derive(Resource)]
pub struct MonsterClips {
    pub attacks: Vec<AttackClip>,
    pub reacts: Vec<ReactClip>,
    pub death: DeathClip,
    /// The leg ratio (kobold model units per DS metre) the extractor scaled the clips by.
    pub scale: f32,
    /// `NpcParam.superArmorDurability`.
    pub poise_max: f32,
    pub reach: Reach,
    pub think: Think,
}

impl MonsterClips {
    /// The index of the move called `name`.
    pub fn attack_index(&self, name: &str) -> Option<usize> {
        self.attacks.iter().position(|a| a.spec.name == name)
    }

    /// The clip pair `id` plays.
    pub fn clips(&self, id: MonsterClip) -> Option<(&wscl::Clip, &wscl::Clip)> {
        match id {
            MonsterClip::Attack(i) => self.attacks.get(usize::from(i)).map(|a| (&a.wow, &a.ds)),
            MonsterClip::React(i) => self.reacts.get(usize::from(i)).map(|r| (&r.wow, &r.ds)),
            MonsterClip::Death => Some((&self.death.wow, &self.death.ds)),
        }
    }
}

/// Which clip of the kit is playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterClip {
    Attack(u8),
    React(u8),
    Death,
}

fn nums<const N: usize>(w: &[&str], line: &str) -> Result<[f32; N], String> {
    let bad = || format!("bad monster line {line:?}");
    let v: Vec<f32> = w
        .iter()
        .map(|s| s.parse::<f32>().ok().filter(|f| f.is_finite()))
        .collect::<Option<_>>()
        .ok_or_else(bad)?;
    v.try_into().map_err(|_| bad())
}

/// Reads the kit from the manifest `text` and the clips under `dir`; `Err` says why the kit is
/// unusable (a line missing or malformed, a clip missing, bone counts that disagree).
pub fn parse(text: &str, dir: &Path) -> Result<MonsterClips, String> {
    let read = |sub: &str, name: &str| crate::dsclip::read_clip(dir, sub, name);
    let (mut scale, mut poise_max, mut chr) = (None, None, None);
    let (mut attacks, mut reacts, mut death) = (Vec::new(), Vec::new(), None);
    let (mut reach, mut think) = (None, None);
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        let bad = || format!("bad monster line {line:?}");
        match w.first().copied() {
            Some("monster") => {
                let [_, "kobold", "chr", c, "npc", _, "hp", _, "poise", p, "scale", s] = w[..]
                else {
                    return Err(bad());
                };
                chr = Some(c.to_string());
                poise_max = Some(p.parse::<f32>().map_err(|_| bad())?);
                scale = Some(s.parse::<f32>().map_err(|_| bad())?);
            }
            Some("attack") => {
                let [_, name, clip, "judge", _, "phys", phys, "poise", poise, "level", level, "radius", r1, "radius2", r2] =
                    w[..]
                else {
                    return Err(bad());
                };
                attacks.push((
                    name.to_string(),
                    clip.to_string(),
                    phys.parse::<f32>().map_err(|_| bad())?,
                    poise.parse::<f32>().map_err(|_| bad())?,
                    level.parse::<u8>().map_err(|_| bad())?,
                    r1.parse::<f32>().map_err(|_| bad())?,
                    r2.parse::<f32>().map_err(|_| bad())?,
                ));
            }
            Some("react") => {
                let [_, level, clip, "push", fwd, left] = w[..] else {
                    return Err(bad());
                };
                let [f, l] = nums::<2>(&[fwd, left], line)?;
                reacts.push((level.parse::<u8>().map_err(|_| bad())?, clip.to_string(), Vec2::new(f, l)));
            }
            Some("death") => {
                let [_, clip] = w[..] else { return Err(bad()) };
                death = Some(clip.to_string());
            }
            Some("think") => {
                let [_, "near", a, "mid", b, "far", c, "out", d] = w[..] else {
                    return Err(bad());
                };
                let [near, mid, far, out] = nums::<4>(&[a, b, c, d], line)?;
                think = Some(Think { near, mid, far, out });
            }
            Some("reach") => {
                let [_, bone, x, y, z] = w[..] else {
                    return Err(bad());
                };
                let [x, y, z] = nums::<3>(&[x, y, z], line)?;
                reach = Some(Reach {
                    bone: bone.parse().map_err(|_| bad())?,
                    axis: Vec3::new(x, y, z),
                });
            }
            _ => {}
        }
    }
    let chr = chr.ok_or("manifest has no monster line")?;
    let scale = scale.ok_or("manifest has no monster scale")?;
    let mut bones = None;
    let mut check = |wow: &wscl::Clip| match bones {
        None => {
            bones = Some(wow.bones());
            Ok(())
        }
        Some(n) if n == wow.bones() => Ok(()),
        Some(n) => Err(format!("{}: {} bones, the kit's other clips have {n}", wow.name, wow.bones())),
    };
    let mut attack_clips = Vec::new();
    for (name, clip, phys, poise, level, r1, r2) in attacks {
        let wow = read("kobold", &clip)?;
        let ds = read(&chr, &clip)?;
        check(&wow)?;
        let hit = wow
            .hits
            .first()
            .map(|h| (h.start, h.end))
            .ok_or_else(|| format!("{clip} has no hit window"))?;
        let spec = AttackSpec {
            name,
            length: wow.duration(),
            hit,
            damage: (phys * DAMAGE_K).round(),
            poise,
            level,
            radius: r1,
            radius2: r2,
        };
        attack_clips.push(AttackClip { wow, ds, spec });
    }
    if attack_clips.is_empty() {
        return Err("manifest has no attack lines".into());
    }
    let mut react_clips = Vec::new();
    for (level, clip, push) in reacts {
        let wow = read("kobold", &clip)?;
        let ds = read(&chr, &clip)?;
        check(&wow)?;
        react_clips.push(ReactClip { level, wow, ds, push });
    }
    if react_clips.is_empty() {
        return Err("manifest has no react lines".into());
    }
    let death = death.ok_or("manifest has no death line")?;
    let death = DeathClip {
        wow: read("kobold", &death)?,
        ds: read(&chr, &death)?,
    };
    check(&death.wow)?;
    Ok(MonsterClips {
        attacks: attack_clips,
        reacts: react_clips,
        death,
        scale,
        poise_max: poise_max.ok_or("manifest has no monster poise")?,
        reach: reach.ok_or("manifest has no reach line")?,
        think: think.ok_or("manifest has no think line")?,
    })
}

/// Reads the kit from `dir`: a checked manifest, then [`parse`].
pub fn load(dir: &Path) -> Result<MonsterClips, String> {
    let text = crate::dsclip::checked_manifest(dir)?;
    parse(&text, dir)
}

fn startup(mut commands: Commands) {
    match load(&crate::dsclip::cache_dir()) {
        Ok(k) => {
            for a in &k.attacks {
                let s = &a.spec;
                info!(
                    "monster: {} {} ({:.3} s, hit {:.3}-{:.3}, damage {:.0}, poise {:.0}, level {}, radius {:.2})",
                    s.name, a.wow.name, s.length, s.hit.0, s.hit.1, s.damage, s.poise, s.level, s.radius
                );
            }
            info!(
                "monster: {} flinch clips, death {}, poise {:.0}, scale {:.3}",
                k.reacts.len(),
                k.death.wow.name,
                k.poise_max,
                k.scale
            );
            commands.insert_resource(k);
        }
        Err(why) => warn!("monster: no Dark Souls kobold ({why}); the old kobold fights"),
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, startup);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clip of `bones` bones, one second at 10 fps, with an optional hit window.
    fn clip(name: &str, bones: usize, hit: Option<(f32, f32)>) -> wscl::Clip {
        wscl::Clip {
            name: name.into(),
            fps: 10.0,
            model_space: false,
            parents: vec![-1; bones],
            driven: vec![true; bones],
            frames: vec![vec![wscl::Trs::IDENTITY; bones]; 11],
            root_motion: vec![[0.0; 4]; 11],
            flags: Vec::new(),
            hits: hit
                .map(|(start, end)| wscl::HitWindow { judge: 1, start, end })
                .into_iter()
                .collect(),
        }
    }

    fn put(dir: &Path, sub: &str, c: &wscl::Clip) {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
        c.write(&mut std::fs::File::create(dir.join(sub).join(format!("{}.wscl", c.name))).unwrap())
            .unwrap();
    }

    const MANIFEST: &str = "\
monster kobold chr c2060 npc 206000 hp 203 poise 0 scale 0.6180
attack small_1 a00_3000 judge 100 phys 138 poise 10 level 1 radius 0.30 radius2 0.00
attack big a00_3002 judge 120 phys 195 poise 20 level 2 radius 0.30 radius2 0.00
react 1 a00_2000 push -0.300 0.050
react 2 a00_2007 push -0.500 0.000
death a00_9000
think near 2.0 mid 5.0 far 10.0 out 20.0
reach 53 0.0 1.0 0.0
";

    fn kit_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("wow-souls-monster-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (c, hit) in [
            ("a00_3000", Some((0.3, 0.4))),
            ("a00_3002", Some((0.5, 0.6))),
            ("a00_2000", None),
            ("a00_2007", None),
            ("a00_9000", None),
        ] {
            put(&dir, "kobold", &clip(c, 4, hit));
            put(&dir, "c2060", &clip(c, 6, hit));
        }
        dir
    }

    #[test]
    fn a_whole_kit_loads_with_its_numbers() {
        let dir = kit_dir("whole");
        let k = parse(MANIFEST, &dir).unwrap_or_else(|e| panic!("{e}"));
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(k.attacks.len(), 2);
        let s = &k.attacks[0].spec;
        assert_eq!((s.name.as_str(), s.hit, s.level), ("small_1", (0.3, 0.4), 1));
        assert!((s.length - 1.0).abs() < 1.0e-6);
        // 138 · K = 25 exactly; 195 · K = 35.3.
        assert_eq!(s.damage, 25.0);
        assert_eq!(k.attacks[1].spec.damage, 35.0);
        assert_eq!(k.attack_index("big"), Some(1));
        assert_eq!(k.attack_index("nope"), None);
        assert_eq!((k.reacts[0].level, k.reacts[0].push), (1, Vec2::new(-0.3, 0.05)));
        assert_eq!(k.death.wow.name, "a00_9000");
        assert_eq!((k.poise_max, k.scale), (0.0, 0.618));
        assert_eq!(k.reach, Reach { bone: 53, axis: Vec3::Y });
        assert_eq!(k.think.near, 2.0);
        assert!(k.clips(MonsterClip::Attack(1)).is_some() && k.clips(MonsterClip::Attack(2)).is_none());
        assert!(k.clips(MonsterClip::Death).is_some());
    }

    #[test]
    fn an_incomplete_or_malformed_kit_is_refused() {
        let dir = kit_dir("bad");
        for (why, text) in [
            ("no monster line", MANIFEST.replace("monster kobold", "x kobold")),
            ("no death line", MANIFEST.replace("death a00_9000\n", "")),
            ("no react lines", MANIFEST.replace("react 1 a00_2000 push -0.300 0.050\nreact 2 a00_2007 push -0.500 0.000\n", "")),
            ("no reach line", MANIFEST.replace("reach 53 0.0 1.0 0.0\n", "")),
            ("bad monster line", MANIFEST.replace("phys 138", "phys x")),
            ("a00_3000 has no hit window", MANIFEST.replace("a00_3000", "a00_2000")),
        ] {
            let err = parse(&text, &dir).err().unwrap_or_else(|| panic!("{why} loads"));
            assert!(err.contains(why), "{why}: {err}");
        }
        std::fs::remove_dir_all(&dir).ok();
        // A clip with another bone count than the rest.
        let dir = kit_dir("bones");
        put(&dir, "kobold", &clip("a00_2007", 5, None));
        let err = parse(MANIFEST, &dir).err().unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert!(err.contains("5 bones"), "{err}");
    }
}
```
Add `mod monster;` to `main.rs` and `monster::plugin` to the plugin tuple where `dsclip::plugin` is added (`hits.rs` `plugin` adds `crate::hitbody::plugin, crate::dsclip::plugin, crate::dsview::plugin` in `add_plugins((...))`: add `crate::monster::plugin` there). In `dsclip.rs` make `cache_dir` `pub(crate)` (`pub(crate) fn cache_dir()`), and `read_clip`, `checked_manifest` `pub`.

- [ ] **Step 3: Run.** `cargo test -j 4 -p wow-souls monster:: dsclip::` Expected: the two new tests PASS (the first compile fails until `read_clip` etc. exist: do Step 1 first).

- [ ] **Step 4: Checkpoint.** `cargo test -j 4 -p wow-souls` (188 + 2 = 190), fmt, clippy.

### Task 8: the kobold's pose system and the side-by-side

**Files:** Modify `monster.rs` (second half), `dsview.rs`, `npc.rs` (components only).

- [ ] **Step 1: Components in `monster.rs`.** Add (above `plugin`):

```rust
use benilla_world::rig_anim::{PosePost, RigPose};

use crate::dsclip::{layers, sample, Fade};

/// The kit clip a creature is playing and its time (virtual seconds: a hit-stop holds it).
#[derive(Component, Debug, Default)]
pub struct MonsterPlay {
    pub clip: Option<(MonsterClip, f32)>,
}

/// The pose blend state: the last clip's end pose fading under the next.
#[derive(Component, Default)]
pub struct MonsterFade(pub Fade<MonsterClip>);

/// While a creature plays a kit clip, the clip owns the rig's driven bones, blended over the rig's
/// own pose at its ends; a death holds its last pose (the clip's time keeps running and the
/// sample clamps to the last frame).
fn pose(
    real: Res<Time<bevy::time::Real>>,
    kit: Option<Res<MonsterClips>>,
    mut creatures: Query<(&MonsterPlay, &mut MonsterFade, &mut RigPose)>,
) {
    let Some(kit) = kit else {
        return;
    };
    for (play, mut fade, mut pose) in &mut creatures {
        let now = play.clip;
        let layers = layers(&mut fade.0, now, real.delta_secs());
        if layers.iter().all(Option::is_none) {
            continue;
        }
        for (id, t, w) in layers.into_iter().flatten() {
            let Some((wow, _)) = kit.clips(id) else {
                continue;
            };
            if wow.bones() != pose.locals.len() {
                continue;
            }
            for (i, local) in pose.locals.iter_mut().enumerate() {
                if !wow.driven[i] {
                    continue;
                }
                let (tr, r, s) = sample(wow, t, i);
                *local = if w >= 1.0 {
                    Transform { translation: tr, rotation: r, scale: s }
                } else {
                    Transform {
                        translation: local.translation.lerp(tr, w),
                        rotation: local.rotation.slerp(r, w),
                        scale: local.scale.lerp(s, w),
                    }
                };
            }
        }
        pose.pose_dirty = true;
    }
}

/// Runs each creature's kit clip clock; the side-by-side (below) loops it instead.
fn advance(time: Res<Time>, mut creatures: Query<&mut MonsterPlay>) {
    for mut p in &mut creatures {
        if let Some((_, t)) = &mut p.clip {
            *t += time.delta_secs();
        }
    }
}
```
and in `plugin`: `app.add_systems(Startup, startup).add_systems(Update, advance).add_systems(PostUpdate, pose.in_set(PosePost));`. The `layers` function requires `K: Copy + PartialEq` (✓: `MonsterClip` is `Copy + PartialEq + Eq`) and `Fade<K>` has a private-field `Default` impl (✓ `impl<K> Default for Fade<K>`). In `dsclip.rs` the items `layers`, `sample`, `Fade` are already `pub`.

Test (add to `monster.rs` tests): the kobold's clip holds the driven bones and leaves the others:

```rust
    #[test]
    fn a_death_holds_its_last_pose_and_undriven_bones_are_left_alone() {
        let mut c = clip("a00_9000", 2, None);
        c.driven = vec![true, false];
        c.frames[10][0].t = [0.0, 5.0, 0.0];
        let (t, _, _) = crate::dsclip::sample(&c, 99.0, 0);
        assert_eq!(t.y, 5.0, "clamped to the last frame: the pose holds");
        assert!(!c.driven[1]);
    }
```
(`sample` is the existing helper; the test only pins the hold-on-clamp contract the death relies on.)

- [ ] **Step 2: `npc.rs`: give the kobold the components.** In `spawn` add `crate::monster::MonsterPlay::default(), crate::monster::MonsterFade::default(),` to the spawned tuple (bevy tuples allow up to 15: count the existing tuple; if it exceeds, wrap the new ones in a nested bundle tuple `(MonsterPlay::default(), MonsterFade::default())`).

- [ ] **Step 3: `dsview.rs`: `WOWSOULS_DSVIEW=kobold:<clip>`.** Add:

```rust
/// `WOWSOULS_DSVIEW=kobold:<clip>`: the kobold plays a kit clip in place, looping, beside the DS
/// ghoul's skeleton (gold gizmo bones, 1.2 yd to its right), the same time on both. `<clip>` is a
/// move's name (`small_1`) or any kit clip's name (`a00_3000`, `a00_2007`, `a00_9000`).
#[derive(Resource, Debug)]
pub struct KoboldView {
    want: String,
    clip: Option<crate::monster::MonsterClip>,
}

impl KoboldView {
    pub fn active(&self) -> bool {
        true
    }
}
```
In `plugin`, before the existing `if let Ok(want) = ...`, branch: `if let Some(rest) = std::env::var("WOWSOULS_DSVIEW").ok().and_then(|v| v.strip_prefix("kobold:").map(str::to_string))` → `app.insert_resource(KoboldView { want: rest, clip: None }).add_systems(Update, (kobold_view).chain()).add_systems(PostUpdate, kobold_draw.after(BillboardPlace)); return;` and make the existing branch skip when the value starts with `kobold:` (the `resolve` would otherwise warn): wrap: `if let Ok(want) = std::env::var("WOWSOULS_DSVIEW").filter(|v| !v.starts_with("kobold:"))`.

```rust
/// Finds the clip, then keeps the kobold playing it in a loop (the clip's own length).
fn kobold_view(
    kit: Option<Res<crate::monster::MonsterClips>>,
    mut view: ResMut<KoboldView>,
    mut npcs: Query<&mut crate::monster::MonsterPlay>,
    mut warned: Local<bool>,
) {
    let Some(kit) = kit else {
        return;
    };
    if view.clip.is_none() {
        let id = (0..kit.attacks.len())
            .map(|i| crate::monster::MonsterClip::Attack(i as u8))
            .chain((0..kit.reacts.len()).map(|i| crate::monster::MonsterClip::React(i as u8)))
            .chain([crate::monster::MonsterClip::Death])
            .find(|&id| {
                let named = match id {
                    crate::monster::MonsterClip::Attack(i) => kit.attacks[usize::from(i)].spec.name == view.want,
                    _ => false,
                };
                named || kit.clips(id).is_some_and(|(w, _)| w.name == view.want)
            });
        view.clip = id;
        if id.is_none() && !*warned {
            *warned = true;
            warn!("dsview: no kobold clip is called {:?}", view.want);
        }
    }
    let Some(id) = view.clip else {
        return;
    };
    let Some((wow, _)) = kit.clips(id) else {
        return;
    };
    let len = wow.duration().max(1.0e-3);
    for mut p in &mut npcs {
        let t = match p.clip {
            Some((c, t)) if c == id => t % len,
            _ => 0.0,
        };
        p.clip = Some((id, t));
    }
}

fn kobold_draw(
    kit: Option<Res<crate::monster::MonsterClips>>,
    view: Res<KoboldView>,
    npcs: Query<(&Transform, &crate::monster::MonsterPlay)>,
    mut gizmos: Gizmos,
) {
    let (Some(kit), Some(id)) = (kit, view.clip) else {
        return;
    };
    let Some((_, ds)) = kit.clips(id) else {
        return;
    };
    let gold = Color::srgb(0.95, 0.75, 0.2);
    for (body, play) in &npcs {
        let Some((_, t)) = play.clip else {
            continue;
        };
        let origin = body.translation + *body.right() * 1.2;
        let at = |b: usize| origin + body.rotation * (sample(ds, t, b).0 * body.scale.y);
        for (i, &p) in ds.parents.iter().enumerate() {
            let joint = at(i);
            gizmos.sphere(Isometry3d::from_translation(joint), 0.02, gold);
            if let Ok(p) = usize::try_from(p) {
                gizmos.line(at(p), joint, gold);
            }
        }
    }
}
```
While the view runs the kobold's AI must not fight it: in `npc.rs::think` add at the top of the loop body `if crate::dsview::kobold_view_on() { continue; }` with in `dsview.rs`: `pub fn kobold_view_on() -> bool { static ON: OnceLock<bool> ...; std::env::var("WOWSOULS_DSVIEW").is_ok_and(|v| v.starts_with("kobold:")) }` (use the same `OnceLock` pattern as `trace_on`).

- [ ] **Step 4: Build and look.** `cargo build -j 4 -p wow-souls`, then a still, close-up shot of every move (the kobold 2.5 yd in front of the player, an observer camera looking at it; the ghoul skeleton stands to its right). One run per clip, e.g.

```bash
for c in small_1 big run_strike body_check pounce; do
  WOWSOULS_DSVIEW=kobold:$c WOWSOULS_KOBOLD_AT=-8909,-136 WOWSOULS_AT=-8913,-136 WOWSOULS_CHECK=10 \
  WOWSOULS_OBSERVE=-3,1.4,-2 WOWSOULS_SHOT="C:/Users/jacob/Documents/GitHub/wow darksouls/docs/evidence/kobold-$c.png" WOWSOULS_SHOT_AT=6 \
  timeout 120 ./target/debug/wow-souls.exe
done
```
(Adjust `WOWSOULS_OBSERVE`/`_AT`/`_KOBOLD_AT` until both figures are in frame; see `docs/HANDOVER.md`'s switch list.) Read each picture with the Read tool. Also run the reaction clips (`a00_2000`, `a00_2007`) and the death (`a00_9000`).

- [ ] **Step 5: STOP: show the user the look.** Send the pictures (SendUserFile) and the extractor's gate numbers. **Do not start Task 9 before the user has said the look is good enough** (the spec's order of work). If they dislike a pose, the fix is in `bonemap.rs`'s `KOBOLD` pairs (`align` flags, aim bones) and a re-extract, not in the game.

- [ ] **Step 6: Checkpoint.** `cargo test -j 4 --workspace`, fmt, clippy.

---

## Phase 3: the fight

### Task 9: pattern table (pure)

**Files:** Create `crates/wow-souls/src/pattern.rs`; modify `main.rs`.

- [ ] **Step 1: Failing tests first, then the module.** Create `pattern.rs`:

```rust
//! The kobold's pattern: which move it starts and when, as a hand-written table built from the
//! ghoul's real moves (the ghoul's Lua AI is not read; this stands in for it and is replaced,
//! not extended). Pure: the caller owns the random numbers.
//!
//! The kobold *wishes* for a move when it starts closing in (`wish`: weighted over the rows that
//! may open a fight) and starts it once the player is inside that move's band (`choose`); if the
//! player is already inside another band it picks from the rows that fit. After a move it may
//! chain into a follow-up (`follow_up`).

/// One row: a move by name, the player distances (yd, centre to centre) it can start from, its
/// weight as an opener (0: only as a follow-up), and a follow-up with its chance in percent.
pub struct Row {
    pub name: &'static str,
    pub band: (f32, f32),
    pub weight: u32,
    pub then: Option<(&'static str, u32)>,
}

/// Starting table, tuned live. The ghoul's `NpcThinkParam` bands are near 2 m, mid 5, far 10; the
/// kobold's legs are 0.62 of the ghoul's and its lunges follow, so the bands are in kobold yards.
pub const KOBOLD: &[Row] = &[
    Row { name: "small_1", band: (0.0, 2.0), weight: 4, then: Some(("small_2", 40)) },
    Row { name: "small_2", band: (0.0, 2.4), weight: 0, then: Some(("big", 30)) },
    Row { name: "big", band: (0.0, 2.0), weight: 2, then: None },
    Row { name: "body_check", band: (0.0, 1.7), weight: 1, then: None },
    Row { name: "run_strike", band: (2.2, 4.5), weight: 3, then: None },
    Row { name: "pounce", band: (2.2, 4.0), weight: 1, then: None },
];

/// The farthest any row starts from.
pub fn reach(table: &[Row]) -> f32 {
    table.iter().map(|r| r.band.1).fold(0.0, f32::max)
}

/// A small xorshift generator: repeatable runs (`WOWSOULS_SEED`) and tests.
#[derive(Debug, Clone, Copy)]
pub struct Rng(pub u64);

impl Rng {
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 32) as u32
    }

    /// A number in `0..n` (`n` > 0).
    pub fn below(&mut self, n: u32) -> u32 {
        self.next_u32() % n.max(1)
    }
}

/// The weighted pick among `candidates` (row index, weight); `None` when every weight is 0.
fn weighted(candidates: &[(usize, u32)], rng: &mut Rng) -> Option<usize> {
    let total: u32 = candidates.iter().map(|c| c.1).sum();
    if total == 0 {
        return None;
    }
    let mut roll = rng.below(total);
    for &(i, w) in candidates {
        if roll < w {
            return Some(i);
        }
        roll -= w;
    }
    None
}

/// The move it wishes for next, weighted over the openers; the move it just made weighs half.
pub fn wish(table: &[Row], last: Option<usize>, rng: &mut Rng) -> Option<usize> {
    let c: Vec<(usize, u32)> = table
        .iter()
        .enumerate()
        .map(|(i, r)| (i, if Some(i) == last { r.weight / 2 } else { r.weight }))
        .collect();
    weighted(&c, rng)
}

/// The move to start with the player `dist` away: the wish if its band holds `dist`, else a
/// weighted pick among the openers whose band does; `None` when nothing fits yet.
pub fn choose(
    table: &[Row],
    dist: f32,
    wish: Option<usize>,
    last: Option<usize>,
    rng: &mut Rng,
) -> Option<usize> {
    let fits = |r: &Row| dist >= r.band.0 && dist <= r.band.1;
    if let Some(w) = wish.filter(|&w| table.get(w).is_some_and(fits)) {
        return Some(w);
    }
    let c: Vec<(usize, u32)> = table
        .iter()
        .enumerate()
        .filter(|(_, r)| fits(r))
        .map(|(i, r)| (i, if Some(i) == last { r.weight / 2 } else { r.weight }))
        .collect();
    weighted(&c, rng)
}

/// The follow-up of `prev`, if its chance comes up and the player is still inside its band.
pub fn follow_up(table: &[Row], prev: usize, dist: f32, rng: &mut Rng) -> Option<usize> {
    let (name, percent) = table.get(prev)?.then?;
    let next = table.iter().position(|r| r.name == name)?;
    let band = table[next].band;
    (dist >= band.0 && dist <= band.1 && rng.below(100) < percent).then_some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(name: &str) -> usize {
        KOBOLD.iter().position(|r| r.name == name).unwrap()
    }

    #[test]
    fn the_table_names_the_six_moves_and_its_follow_ups_exist() {
        assert_eq!(KOBOLD.len(), 6);
        for r in KOBOLD {
            if let Some((n, p)) = r.then {
                assert!(KOBOLD.iter().any(|x| x.name == n), "{n}");
                assert!(p <= 100);
            }
        }
        assert_eq!(KOBOLD[index("small_2")].weight, 0, "small_2 only follows small_1");
        assert_eq!(reach(KOBOLD), 4.5);
    }

    #[test]
    fn a_wish_is_an_opener_and_the_last_move_is_less_likely() {
        let mut rng = Rng(7);
        let mut counts = vec![0u32; KOBOLD.len()];
        for _ in 0..4000 {
            counts[wish(KOBOLD, None, &mut rng).unwrap()] += 1;
        }
        assert_eq!(counts[index("small_2")], 0);
        // Weights 4 : 2 : 1 : 3 : 1 out of 11.
        assert!(counts[index("small_1")] > counts[index("big")]);
        assert!(counts[index("run_strike")] > counts[index("pounce")]);
        let mut again = vec![0u32; KOBOLD.len()];
        let last = Some(index("small_1"));
        for _ in 0..4000 {
            again[wish(KOBOLD, last, &mut rng).unwrap()] += 1;
        }
        assert!(again[index("small_1")] < counts[index("small_1")], "{again:?} vs {counts:?}");
    }

    #[test]
    fn it_starts_the_wish_inside_its_band_and_otherwise_picks_what_fits() {
        let mut rng = Rng(3);
        let run = Some(index("run_strike"));
        // Too far for any band, then inside the wish's.
        assert_eq!(choose(KOBOLD, 6.0, run, None, &mut rng), None);
        assert_eq!(choose(KOBOLD, 3.0, run, None, &mut rng), run);
        // The wish is the far move but the player is already close: a near move.
        for _ in 0..50 {
            let c = choose(KOBOLD, 1.0, run, None, &mut rng).unwrap();
            assert!(matches!(KOBOLD[c].name, "small_1" | "big" | "body_check"), "{}", KOBOLD[c].name);
        }
    }

    #[test]
    fn a_follow_up_needs_its_chance_and_its_band() {
        let mut rng = Rng(11);
        let one = index("small_1");
        let hits = (0..1000)
            .filter(|_| follow_up(KOBOLD, one, 1.5, &mut rng) == Some(index("small_2")))
            .count();
        assert!((300..500).contains(&hits), "{hits} follow-ups in 1000 (40 %)");
        assert_eq!(follow_up(KOBOLD, one, 3.0, &mut rng), None, "the player backed off");
        assert_eq!(follow_up(KOBOLD, index("big"), 1.0, &mut rng), None, "big has none");
    }

    #[test]
    fn the_same_seed_gives_the_same_fight() {
        let run = |seed| {
            let mut rng = Rng(seed);
            (0..20).map(|_| wish(KOBOLD, None, &mut rng).unwrap()).collect::<Vec<_>>()
        };
        assert_eq!(run(5), run(5));
        assert_ne!(run(5), run(6));
    }
}
```
Add `mod pattern;` to `main.rs`.

- [ ] **Step 2: Run.** `cargo test -j 4 -p wow-souls pattern::` PASS (if a statistic bound is off by the generator's luck, widen the range, never the table).

- [ ] **Step 3: Checkpoint.** fmt + clippy (`dead_code` warnings on `pub` items in a binary crate: add `#![allow(dead_code)]` at the top of `pattern.rs` until Task 11 uses them, then remove it).

### Task 10: the hit volume (pure)

**Files:** Create `crates/wow-souls/src/foehit.rs`; modify `main.rs`.

- [ ] **Step 1: Module with tests.**

```rust
//! A creature's blow against the player: the hand's volume (a capsule from the hand along the blade
//! axis, `radius` thick) swept from its last pose to this one against the player's `Hurtbox` axis.
//! Pure; `npc` feeds it the posed hand.

use bevy::prelude::*;

use crate::hits::segment_distance;

/// A volume: its two ends (hand, tip) and its radius (yd).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Volume {
    pub ends: [Vec3; 2],
    pub radius: f32,
}

/// Whether a volume sweeping from `prev` to `now` touches the capsule with axis `cap` and radius
/// `cap_r`: the volume at both ends of the frame and the paths of its two ends and its middle.
pub fn touches(now: Volume, prev: Option<[Vec3; 2]>, cap: (Vec3, Vec3), cap_r: f32) -> bool {
    let reach = cap_r + now.radius;
    let mut segments = vec![(now.ends[0], now.ends[1])];
    if let Some(p) = prev {
        segments.push((p[0], p[1]));
        segments.push((p[0], now.ends[0]));
        segments.push((p[1], now.ends[1]));
        segments.push((p[0].lerp(p[1], 0.5), now.ends[0].lerp(now.ends[1], 0.5)));
    }
    segments
        .into_iter()
        .any(|(a, b)| segment_distance(a, b, cap.0, cap.1).0 <= reach)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player() -> ((Vec3, Vec3), f32) {
        (
            (Vec3::new(0.0, 0.3, 0.0), Vec3::new(0.0, 1.5, 0.0)),
            0.33,
        )
    }

    fn volume(a: Vec3, b: Vec3) -> Volume {
        Volume { ends: [a, b], radius: 0.15 }
    }

    #[test]
    fn a_hand_through_the_body_touches_and_one_out_of_reach_does_not() {
        let (cap, r) = player();
        assert!(touches(volume(Vec3::new(0.3, 1.0, 0.0), Vec3::new(0.3, 1.0, 0.5)), None, cap, r));
        assert!(!touches(volume(Vec3::new(1.5, 1.0, 0.0), Vec3::new(1.5, 1.0, 0.5)), None, cap, r));
    }

    #[test]
    fn a_fast_swing_that_jumps_the_body_between_frames_still_touches() {
        let (cap, r) = player();
        let prev = [Vec3::new(-2.0, 1.0, 0.1), Vec3::new(-2.0, 1.0, 0.6)];
        let now = volume(Vec3::new(2.0, 1.0, 0.1), Vec3::new(2.0, 1.0, 0.6));
        assert!(!touches(now, None, cap, r));
        assert!(touches(now, Some(prev), cap, r));
    }

    #[test]
    fn a_thicker_volume_reaches_further() {
        let (cap, r) = player();
        let at = |radius| Volume { ends: [Vec3::new(0.7, 1.0, 0.0), Vec3::new(0.7, 1.0, 0.4)], radius };
        assert!(!touches(at(0.1), None, cap, r));
        assert!(touches(at(0.5), None, cap, r));
    }
}
```
Add `mod foehit;` to `main.rs`.

- [ ] **Step 2: Run + checkpoint.** `cargo test -j 4 -p wow-souls foehit::` PASS; fmt + clippy (same temporary `allow(dead_code)` note).

### Task 11: the kobold attacks from the clips, chooses by the pattern, strikes with its hand

This task rewrites the kobold's brain. Do it in `npc.rs` in four edits, running the tests after each.

**Files:** Modify `crates/wow-souls/src/npc.rs`; modify `crates/wow-souls/src/monster.rs` (a seeded rng resource and the `WOWSOULS_MOVE` instrument).

- [ ] **Step 1: `Mind`, `Sense`, `think_step` (pure) with their tests.** In `npc.rs`:

```rust
/// What the creature is minded to do.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Mind {
    #[default]
    Idle,
    Chase,
    /// Swinging move `mv` (an index into the kit's attacks; 0 for the procedural swing), `t`
    /// seconds in.
    Attack {
        t: f32,
        mv: u8,
    },
    /// Catching its breath after a swing.
    Cooldown {
        left: f32,
    },
    /// Walking home.
    Return,
}
```
`Sense` gains `pub attack: Option<u8>` (the move it would start now, if any) and `pub cooldown: f32` (the pause after a swing, s). New `think_step`:

```rust
/// One frame of the creature's mind; `attack_len` is the current swing's length (s).
pub fn think_step(mind: &mut Mind, s: Sense, attack_len: f32, dt: f32) -> Cue {
    // A swing, once begun, is seen through.
    if let Mind::Attack { t, .. } = mind {
        *t += dt;
        if *t >= attack_len {
            *mind = Mind::Cooldown { left: s.cooldown };
            return Cue::AttackEnded;
        }
        return Cue::None;
    }
    let stand_down = if s.home_dist > HOME_SLACK {
        Mind::Return
    } else {
        Mind::Idle
    };
    let engaged = s.player_alive && s.dist <= LEASH_RANGE;
    *mind = match (*mind, s.attack) {
        (Mind::Idle | Mind::Return, _) if s.player_alive && s.dist <= AGGRO_RANGE => Mind::Chase,
        (Mind::Idle | Mind::Return, _) => stand_down,
        (_, _) if !engaged => stand_down,
        (Mind::Chase, Some(mv)) if s.facing >= ATTACK_FACING => {
            *mind = Mind::Attack { t: 0.0, mv };
            return Cue::AttackStarted;
        }
        (Mind::Cooldown { left }, _) if left - dt > 0.0 => Mind::Cooldown { left: left - dt },
        _ => Mind::Chase,
    };
    Cue::None
}
```
Update the `Brain`:

```rust
#[derive(Component, Debug, Default)]
pub struct Brain {
    pub mind: Mind,
    /// This swing has landed.
    pub(crate) struck: bool,
    /// This swing has passed through a roll's invulnerability.
    pub(crate) dodged: bool,
    /// The move it wishes for next and the one it made last (kit indices).
    pub wish: Option<u8>,
    pub last: Option<u8>,
}
```
Update the in-file tests: the `sense` helper becomes

```rust
    fn sense(dist: f32) -> Sense {
        Sense {
            dist,
            facing: 1.0,
            player_alive: true,
            home_dist: 0.0,
            attack: (dist <= ATTACK_DISTANCE).then_some(0),
            cooldown: COOLDOWN,
        }
    }
```
and `Sense { player_alive: false, ..away(sense(2.0)) }` etc. keep working through `..`; `askew` keeps `..sense(1.5)`. Add a test that a chosen move rides into the state and an unready move does not start:

```rust
    #[test]
    fn it_starts_the_move_it_was_given_and_waits_without_one() {
        let mut m = Mind::Chase;
        let none = Sense { attack: None, ..sense(1.5) };
        assert_eq!(think_step(&mut m, none, LEN, DT), Cue::None);
        assert_eq!(m, Mind::Chase);
        let go = Sense { attack: Some(4), ..sense(3.0) };
        assert_eq!(think_step(&mut m, go, LEN, DT), Cue::AttackStarted);
        assert_eq!(m, Mind::Attack { t: 0.0, mv: 4 });
        // The cooldown after the swing is the one the sense carries.
        let slow = Sense { cooldown: 0.3, ..sense(3.0) };
        while matches!(m, Mind::Attack { .. }) {
            think_step(&mut m, slow, LEN, DT);
        }
        assert_eq!(m, Mind::Cooldown { left: 0.3 });
    }
```

- [ ] **Step 2: Run** `cargo test -j 4 -p wow-souls npc::` PASS (the system code still fails to compile until Steps 3-4; do the `think` edit now so the crate builds).

- [ ] **Step 3: The attack spec lookup, `think`, and the fallback.** In `npc.rs` add after the consts:

```rust
/// What a swing is: the kit's move, or the old procedural swing (its clip's length at the slowed
/// rate, the strike window measured on it).
fn attack_spec(
    mv: u8,
    kit: Option<&crate::monster::MonsterClips>,
    anims: &ModelAnimations,
) -> crate::monster::AttackSpec {
    match kit.and_then(|k| k.attacks.get(usize::from(mv))) {
        Some(a) => a.spec.clone(),
        None => {
            let len = anims
                .find(KOBOLD_ATTACK)
                .map_or(1.0, |c| c.duration / KOBOLD_ATTACK_RATE);
            crate::monster::AttackSpec {
                name: "swing".into(),
                length: len,
                hit: (STRIKE_FROM * len, STRIKE_TO * len),
                damage: KOBOLD_DAMAGE,
                poise: 0.0,
                level: 1,
                radius: 0.0,
                radius2: 0.0,
            }
        }
    }
}

/// The pause after a swing: the old kobold's, or a short one now that the clips carry their own
/// recovery.
const COOLDOWN_DS: f32 = 0.3;
```
Rewrite `think`'s signature and body in these places (everything not listed stays):

  - add parameters `kit: Option<Res<crate::monster::MonsterClips>>`, `mut rng: ResMut<crate::monster::FoeRng>`, `view: Option<Res<crate::dsview::KoboldView>>` is not needed (use `crate::dsview::kobold_view_on()`), and in the npc query `&mut crate::monster::MonsterPlay`.
  - first line of the loop body after `*intent = MoveIntent::default();`: `if crate::dsview::kobold_view_on() { continue; }`.
  - Replace the `let sense = Sense { .. }; let clip = anims.find(KOBOLD_ATTACK); let attack_len = ...;` block with:

```rust
        let mut sense = Sense {
            dist: to_player.length(),
            facing: to_player.normalize_or_zero().dot(nose),
            player_alive,
            home_dist: to_home.length(),
            attack: None,
            cooldown: COOLDOWN,
        };
        let table = crate::pattern::KOBOLD;
        match kit.as_deref() {
            Some(k) if brain.mind == Mind::Chase => {
                // Wish for a move while closing in; start it once the player is inside its band.
                let rows = |name: &str| k.attack_index(name);
                let wish = *brain.wish.get_or_insert_with(|| {
                    crate::pattern::wish(table, brain.last.and_then(|l| table.iter().position(|r| rows(r.name) == Some(usize::from(l)))), &mut rng.0)
                        .and_then(|i| rows(table[i].name))
                        .map_or(0, |i| i as u8)
                });
                let row_of = |mv: u8| table.iter().position(|r| rows(r.name) == Some(usize::from(mv)));
                sense.attack = crate::pattern::choose(
                    table,
                    sense.dist,
                    row_of(wish),
                    brain.last.and_then(row_of),
                    &mut rng.0,
                )
                .and_then(|i| rows(table[i].name))
                .map(|i| i as u8);
                sense.cooldown = COOLDOWN_DS;
            }
            Some(_) => sense.cooldown = COOLDOWN_DS,
            None => sense.attack = (sense.dist <= ATTACK_DISTANCE).then_some(0),
        }
        let mv = match brain.mind {
            Mind::Attack { mv, .. } => mv,
            _ => sense.attack.unwrap_or(0),
        };
        let spec = attack_spec(mv, kit.as_deref(), anims);
        let attack_len = spec.length;
```
    (The three small closures are verbose on purpose: the table's rows are named, the kit's attacks are indexed; `wish` is a kit index, `choose` works on table rows. A forced move for the instrument: after the `match`, `if let (Some(forced), true) = (crate::monster::forced_move(k), sense.attack.is_some()) { sense.attack = Some(forced) }`: see Step 5.)
  - after `let cue = think_step(...)`, handle both cues:

```rust
        if cue == Cue::AttackEnded {
            brain.last = Some(mv);
            brain.wish = None;
            // A chained move: the player still inside the follow-up's band.
            if let (Some(k), Some(row)) = (
                kit.as_deref(),
                table.iter().position(|r| kit.as_deref().and_then(|k| k.attack_index(r.name)) == Some(usize::from(mv))),
            ) {
                if let Some(next) = crate::pattern::follow_up(table, row, sense.dist, &mut rng.0).and_then(|i| k.attack_index(table[i].name)) {
                    brain.mind = Mind::Attack { t: 0.0, mv: next as u8 };
                    // Falls through to the AttackStarted handling below as if it had just begun.
                    cue_chained = true;
                }
            }
        }
```
    with `let mut cue_chained = false;` declared before and `if cue == Cue::AttackStarted || cue_chained {` replacing the existing start check. Inside, the swing start now: `brain.struck = false; brain.dodged = false; play.clip = kit.as_ref().and(Some((MonsterClip::Attack(mv_now), 0.0)));` where `mv_now` is `match brain.mind { Mind::Attack { mv, .. } => mv, _ => 0 }`, and with a kit the underlay is Stand (`play_once(anims, &mut p, &mut tr, 0, 1.0)` so the undriven bones idle) while without a kit the old `play_once(... KOBOLD_ATTACK, KOBOLD_ATTACK_RATE)`; the info line prints `spec` (name, length, window, damage).
  - `Mind::Attack { t } if t < STRIKE_FROM * attack_len` becomes `Mind::Attack { t, .. } if t < spec.hit.0` (re-fetch `spec` for the *current* move after a chain), and `Mind::Attack { .. } => (Vec3::ZERO, 0.0, Vec3::ZERO)` stays.
  - The lunge: with a kit, set the intent from the clip's root motion after the `*intent = MoveIntent { .. }` assignment:

```rust
        if let (Some(k), Mind::Attack { t, mv }) = (kit.as_deref(), brain.mind) {
            if let Some((wow, _)) = k.clips(crate::monster::MonsterClip::Attack(mv)) {
                let step = (crate::dsclip::progress(wow, t) - crate::dsclip::progress(wow, (t - dt).max(0.0))) * xf.scale.y;
                *intent = crate::combat::root_step(*xf.forward(), step, dt);
            }
        }
```
    (`combat::root_step` is private: make it `pub(crate) fn root_step` in `combat/mod.rs` and re-export `pub(crate) use` if it is in a submodule path.) Also advance nothing else: `MonsterPlay.t` is advanced by `monster::advance`.
  - The old strike block (`let Mind::Attack { t } = brain.mind else { continue; }; ... hurts.write(...)`) stays but is wrapped to run only without a kit: `if kit.is_some() { continue; }` before it, and it reads `Mind::Attack { t, .. }`, `in_strike_window(t, attack_len)`. (The kit's strike is the new system in Step 4.)
  - `Brain` has `wish`/`last`; `reset` sets `*brain = Brain::default()` already clears them; also reset `MonsterPlay`/`MonsterFade` there: add `&mut MonsterPlay, &mut MonsterFade` to its query and `play.clip = None; *fade = MonsterFade::default();`.

`FoeRng` (in `monster.rs`):

```rust
/// The kobold's dice: `WOWSOULS_SEED=<n>` makes a run repeatable.
#[derive(Resource)]
pub struct FoeRng(pub crate::pattern::Rng);

impl Default for FoeRng {
    fn default() -> Self {
        let seed = std::env::var("WOWSOULS_SEED")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        FoeRng(crate::pattern::Rng(seed))
    }
}
```
registered with `app.init_resource::<FoeRng>()` in `monster::plugin`.

- [ ] **Step 4: the strike system.** Add to `npc.rs` and register after `BillboardPlace` in `PostUpdate`:

```rust
/// The kit's blow: through a move's hit window the kobold's hand volume (a capsule from its right
/// hand along the blade axis, the move's radius thick) is swept against the player's hurtbox,
/// once per move; a roll's invulnerability lets it through, as it always did.
#[allow(clippy::type_complexity)]
fn strike(
    kit: Option<Res<crate::monster::MonsterClips>>,
    m2s: Res<Assets<benilla_assets::M2Model>>,
    avatar: Query<(&Transform, &Action, &Hurtbox), (With<Avatar>, Without<Hostile>)>,
    mut npcs: Query<(
        Entity,
        &Brain,
        &GlobalTransform,
        &ActorModel,
        &benilla_world::rig_anim::RigPose,
        &mut crate::monster::HandTrail,
    )>,
    mut brains: Query<&mut Brain, With<Hostile>>,
    mut hurts: MessageWriter<PlayerHurt>,
) {
    let (Some(kit), Ok((player, action, hurt))) = (kit, avatar.single()) else {
        return;
    };
    for (e, brain, at, model, pose, mut trail) in &mut npcs {
        let Mind::Attack { t, mv } = brain.mind else {
            trail.0 = None;
            continue;
        };
        let Some(a) = kit.attacks.get(usize::from(mv)) else {
            continue;
        };
        let Some(hand) = m2s
            .get(&model.m2)
            .and_then(|m| m.attachments.iter().find(|x| x.id == 1).copied())
        else {
            continue;
        };
        let Some(base) = pose.posed_point(at, hand.bone, hand.offset) else {
            continue;
        };
        // The blade axis turned by the hand bone's posed rotation, in world space.
        let Some(bone) = pose.model.get(usize::from(kit.reach.bone)) else {
            continue;
        };
        let scale = at.scale().y;
        let axis = at.affine().transform_vector3(bone.transform_vector3(kit.reach.axis)).normalize_or_zero();
        let tip = base + axis * crate::monster::REACH_LENGTH * scale;
        let now = crate::foehit::Volume {
            ends: [base, tip],
            radius: a.spec.radius.max(a.spec.radius2) * kit.scale * scale,
        };
        let prev = trail.0.replace([base, tip]);
        if brain.struck || t < a.spec.hit.0 || t >= a.spec.hit.1 || action.is_dead() {
            continue;
        }
        let cap = hurt.axis(player.translation);
        if !crate::foehit::touches(now, prev, cap, hurt.radius) {
            continue;
        }
        let Ok(mut b) = brains.get_mut(e) else {
            continue;
        };
        if action.invulnerable() {
            if !b.dodged {
                b.dodged = true;
                info!("npc: kobold's {} passes through the roll (t={t:.2})", a.spec.name);
            }
            continue;
        }
        b.struck = true;
        hurts.write(PlayerHurt { from: e, damage: a.spec.damage });
        info!("npc: kobold's {} strikes the player for {:.0} (t={t:.2})", a.spec.name, a.spec.damage);
    }
}
```
(The two queries over `Brain` conflict if both are in one system: `npcs` takes `&Brain`, `brains` takes `&mut Brain`: Bevy rejects overlapping access. Replace both with one query taking `&mut Brain` and reading it through `brain.mind`; the sketch above separates them only for readability.) Add in `monster.rs`:

```rust
/// The blade volume's length from the hand (kobold yd before the entity scale): a club's.
pub const REACH_LENGTH: f32 = 0.55;

/// The hand volume's last pose, for the sweep.
#[derive(Component, Debug, Default)]
pub struct HandTrail(pub Option<[Vec3; 2]>);
```
add `HandTrail::default()` to the kobold spawn bundle (nest tuples if needed), and register `strike` in `npc::plugin`: `app.add_systems(PostUpdate, strike.after(benilla_world::billboard::BillboardPlace));`. `WOWSOULS_REACH=x,y,z,len` instrument: parse in `monster::startup` into an optional override of `kit.reach.axis` and `REACH_LENGTH` (a `ReachOverride` resource); keep it to the axis and a length factor.

- [ ] **Step 5: `WOWSOULS_MOVE`.** In `monster.rs`:

```rust
/// `WOWSOULS_MOVE=<name>`: the kobold's next attack is this move (a test aid).
pub fn forced_move(kit: &MonsterClips) -> Option<u8> {
    static WANT: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let want = WANT.get_or_init(|| std::env::var("WOWSOULS_MOVE").ok());
    want.as_deref()
        .and_then(|n| kit.attack_index(n))
        .map(|i| i as u8)
}
```
and in `think`, after the `match kit.as_deref() { .. }`: `if let (Some(k), Some(_)) = (kit.as_deref(), sense.attack) { if let Some(f) = crate::monster::forced_move(k) { sense.attack = Some(f); } }` (a forced move still starts only when `choose` says some band fits; set `WOWSOULS_KOBOLD_AT` 1.5 yd away for near moves).

- [ ] **Step 6: Run and look.** `cargo test -j 4 -p wow-souls` then live, flat ground, the kobold sent to 1.5 yd and the player still (the spec's flat-ground recipe: `WOWSOULS_AT=-8906,-136 WOWSOULS_KOBOLD_AT=-8908,-136`), one run per move:

```bash
for mv in small_1 big run_strike body_check pounce; do
  WOWSOULS_AT=-8906,-136 WOWSOULS_KOBOLD_AT=-8909,-136 WOWSOULS_MOVE=$mv WOWSOULS_SEED=1 WOWSOULS_CHECK=12 WOWSOULS_TRACE=1 timeout 120 ./target/debug/wow-souls.exe 2>&1 | grep -E "npc: kobold|combat:|monster:|player: struck" | head -12
done
```
Expected per run: `npc: kobold winds up`, then `npc: kobold's <move> strikes the player for 25|35|34|19|22` (or `passes through` when the roll instrument is on: add `WOWSOULS_DODGE=0.3`), exactly once per swing, at a clip time inside the move's hit window (the `t=` in the line is within the spec's table). Tune `REACH_LENGTH` and the radius scale if a move whiffs at 1.5 yd or hits from 3 yd; the old kobold connected at 1.1-1.6 yd centre distance, so that is the target. Run twice: `WOWSOULS_DODGE=0.3` (the roll at the telegraph passes the first move's strike).

- [ ] **Step 7: Checkpoint.** fmt, clippy `-D warnings` (remove the temporary `allow(dead_code)`s in `pattern.rs`, `foehit.rs`), `cargo test -j 4 --workspace`.

### Task 12: reactions and poise

**Files:** Modify `npc.rs`, `monster.rs`.

- [ ] **Step 1: Poise (pure) and reaction choice (pure), with tests.** In `monster.rs`:

```rust
/// Poise: a number that soaks up blows. A hit takes its poise damage off `left`; the creature
/// flinches when `left` falls to zero or below, then `left` is whole again; it is whole again
/// after [`POISE_RECOVER`] seconds without a hit. With `max` 0 (the ghoul's) every blow flinches.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Poise {
    pub max: f32,
    pub left: f32,
    pub since_hit: f32,
}

pub const POISE_RECOVER: f32 = 5.0;

impl Poise {
    pub fn new(max: f32) -> Poise {
        Poise { max, left: max, since_hit: 0.0 }
    }

    /// Takes a blow's poise damage; whether it makes the creature flinch.
    pub fn hit(&mut self, damage: f32) -> bool {
        self.since_hit = 0.0;
        self.left -= damage;
        if self.left <= 0.0 {
            self.left = self.max;
            true
        } else {
            false
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.since_hit += dt;
        if self.since_hit >= POISE_RECOVER {
            self.left = self.max;
        }
    }
}

/// The flinch clip for a blow of `level` that pushes the creature along `push` (`(forward, left)`
/// in its frame): among the level's clips, the one whose measured push points most nearly the
/// same way; a blow with no direction takes the level's first clip.
pub fn choose_react(kit: &MonsterClips, level: u8, push: Vec2) -> Option<u8> {
    let candidates = kit
        .reacts
        .iter()
        .enumerate()
        .filter(|(_, r)| r.level == level.clamp(1, 2));
    if push.length_squared() < 1.0e-6 {
        return candidates.map(|(i, _)| i as u8).next();
    }
    candidates
        .max_by(|(_, a), (_, b)| {
            let cos = |p: Vec2| p.normalize_or_zero().dot(push.normalize());
            cos(a.push).total_cmp(&cos(b.push))
        })
        .map(|(i, _)| i as u8)
}
```
Tests (add to `monster.rs`):

```rust
    #[test]
    fn poise_zero_always_flinches_and_poise_soaks_a_weak_blow() {
        let mut ghoul = Poise::new(0.0);
        assert!(ghoul.hit(0.0) && ghoul.hit(10.0));
        let mut stout = Poise::new(30.0);
        assert!(!stout.hit(10.0), "20 left");
        assert!(!stout.hit(10.0), "10 left");
        assert!(stout.hit(10.0), "0 left: it breaks and is whole again");
        assert_eq!(stout.left, 30.0);
        stout.hit(10.0);
        stout.tick(POISE_RECOVER);
        assert_eq!(stout.left, 30.0, "rested");
    }

    #[test]
    fn a_blow_from_the_front_takes_the_clip_that_throws_the_head_back() {
        let dir = kit_dir("react");
        let k = parse(MANIFEST, &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        // Level 1 has the one clip (a00_2000, push back); level 2 the other.
        assert_eq!(choose_react(&k, 1, Vec2::new(-1.0, 0.0)), Some(0));
        assert_eq!(choose_react(&k, 2, Vec2::new(-1.0, 0.0)), Some(1));
        assert_eq!(choose_react(&k, 2, Vec2::ZERO), Some(1));
        assert_eq!(choose_react(&k, 9, Vec2::new(-1.0, 0.0)), Some(1), "levels above 2 clamp");
    }
```

- [ ] **Step 2: `npc.rs` `react`.** New signature adds `kit: Option<Res<MonsterClips>>`, query items `&mut Poise`, `&mut MonsterPlay`, `&Transform`, and `&GlobalTransform` is not needed. The body with a kit:

```rust
        if hit.lethal {
            // as before: underlay the WoW death, the kit's death on top
            play_once(anims, &mut p, &mut tr, DEATH, 1.0);
            if kit.is_some() {
                play.clip = Some((MonsterClip::Death, 0.0));
            }
            // (rest as before)
        } else {
            let flinch = kit.as_deref().map_or(true, |_| poise.hit(hit.poise));
            if flinch {
                let push = {
                    let away = Vec3::new(xf.translation.x - hit.from.x, 0.0, xf.translation.z - hit.from.z).normalize_or_zero();
                    let local = xf.rotation.inverse() * away;
                    Vec2::new(-local.z, -local.x)
                };
                let (len, clip) = match kit.as_deref().and_then(|k| choose_react(k, hit.level, push).map(|i| (k, i))) {
                    Some((k, i)) => {
                        play_once(anims, &mut p, &mut tr, COMBAT_WOUND, 1.0);
                        (k.reacts[usize::from(i)].wow.duration(), Some(MonsterClip::React(i)))
                    }
                    None => (play_once(anims, &mut p, &mut tr, COMBAT_WOUND, 1.0).unwrap_or(0.4), None),
                };
                play.clip = clip.map(|c| (c, 0.0));
                *reaction = Reaction::Flinch(len);
                if matches!(brain.mind, Mind::Attack { .. }) {
                    info!("npc: kobold's swing broken by the hit");
                }
                brain.mind = Mind::Chase;
                brain.wish = None;
                info!("npc: kobold flinches (level {}, push {push:?}, {len:.2} s)", hit.level);
            } else {
                info!("npc: kobold holds its ground (poise {:.0}/{:.0})", poise.left, poise.max);
            }
        }
```
(`xf` is the kobold's `&Transform` (add to the query); `hit.from` is the player's position.) Add a system that ticks poise: `fn poise_tick(time: Res<Time>, mut q: Query<&mut Poise>) { for mut p in &mut q { p.tick(time.delta_secs()); } }` registered in the `Update` chain before `react`. Insert `Poise::new(kit_poise)` at spawn: `spawn` receives `kit: Option<Res<MonsterClips>>` and uses `Poise::new(kit.map_or(0.0, |k| k.poise_max))`. `reset` calls `*poise = Poise::new(poise.max)`. When the kobold is dead the death clip's `MonsterPlay` keeps its clock; `monster::advance` caps nothing (the sample clamps).

- [ ] **Step 3: Run and look.** `cargo test -j 4 -p wow-souls`. Live: the player kills the kobold with R1 and R2 (`WOWSOULS_ATTACK=3,0.9`, `WOWSOULS_HEAVY=...`, kobold 2 yd ahead, `WOWSOULS_TRACE=1`), expected log: `npc: kobold flinches (level 1, push (-1.00, ...), 1.00 s)` for R1, `(level 2 ...)` for R2, the kill `npc: kobold dies`, the death pose holding (`dead[0] ...` trace lines, root visible), then `Respawned` → `npc: creatures reset at home`, the loop repeating. Screenshot a flinch and the death: `WOWSOULS_SHOT_AT`.

- [ ] **Step 4: Checkpoint.** fmt, clippy, `cargo test -j 4 --workspace`.

### Task 13: the fight as a whole, gates, installs, docs

- [ ] **Step 1: A seeded fight.** `WOWSOULS_SEED=7 WOWSOULS_CHECK=60 WOWSOULS_TRACE=1 WOWSOULS_DODGE=0.3 WOWSOULS_AT=-8906,-136 WOWSOULS_KOBOLD_AT=-8920,-136 timeout 150 ./target/debug/wow-souls.exe`: the log shows several different moves, never overlapping (`npc: kobold winds up` lines at least a move length apart), `Telegraph` once per move, chained `small_2` after some `small_1`, and no ECS faults (`WOWSOULS_CHECK` exits 0).

- [ ] **Step 2: No cache.** `WOWSOULS_DSR_CACHE=/nonexistent WOWSOULS_CHECK=15 WOWSOULS_TRACE=1 WOWSOULS_KOBOLD_AT=-8909,-136 ...`: one `monster: no Dark Souls kobold (...)` warning, the old kobold fights (`npc: kobold winds up (1.43 s, strike 0.49-0.69 s)`, 25 damage).

- [ ] **Step 3: Gates.** `cargo fmt --all -- --check`; `cargo clippy -j 4 --workspace --all-targets -- -D warnings`; `cargo test -j 4 --workspace` with and without the install env. Expected totals: wow-souls ≥ 190 + the new tests, dsr-formats 29, dsr-cache 17 + 8 or so, wscl 4.

- [ ] **Step 4: Installs untouched.** Snapshot before and after, as `HANDOVER.md` describes (`find ... -printf '%p %s %T@\n' | sort`, WoW without `realmlist.wtf`), diff empty. The first snapshot is taken **before Task 5's extractor run**, i.e. at the start of the whole plan; if that was not done, take one now and compare modification times against the plan's start (the extractor only reads).

- [ ] **Step 5: Docs.** Update `docs/HANDOVER.md`: "Next action" becomes the status of E (what is built, the numbers from Steps 1-2, what the user must judge: the kobold's look in every move, the feel of the pattern, the damage scale K, the reaction variants), the architecture table gets rows for `monster.rs`, `pattern.rs`, `foehit.rs`, the switch list gets `WOWSOULS_DSVIEW=kobold:<clip>`, `WOWSOULS_MOVE`, `WOWSOULS_SEED`, `WOWSOULS_REACH`. Update the spec's two corrections: in "Extractor / `monster.rs`" replace "and each judge's `AtkParam_Npc` row name against the record" with "and each judge resolves to an attack row"; in "Pattern" replace "bands follow `NpcThinkParam` ... scaled by the same `scale`" with "bands start from `NpcThinkParam`'s 2/5/10 m and are tuned in kobold yards (`pattern::KOBOLD`)". Update the memory note `wow-souls-combat-plan`.

- [ ] **Step 6: Report to the user** with the pictures (`docs/evidence/kobold-*.png`), the trace lines and what is left for their eyes.

---

## Self-review

**Spec coverage.** Criterion 1 (numbers equal data): Task 4 (params), Task 5 (windows, lunge, install test), Task 11 Step 6 (live traces). Criterion 2 (look, side by side, gates): Task 3 gates, Task 8 `DSVIEW=kobold:`. Criterion 3 (pattern, telegraph, forced move): Tasks 9 and 11 (`WOWSOULS_MOVE`, `WOWSOULS_SEED`). Criterion 4 (reactions, death, poise): Task 12. Criterion 5 (`think_step`, `Telegraph`, `PlayerHurt`, `WOWSOULS_DODGE`, roll pass-through): Task 11 (`strike` keeps the `invulnerable` branch). Criterion 6 (no cache; gates; installs): Task 13. Non-goals untouched. Reach and `WOWSOULS_REACH`: Task 11 Step 4. Hit volume spec: Tasks 10-11. Extractor spec items: Tasks 1-5 (checks for reaction direction replaced by the *measured* push, which the spec's wording allowed: "fails naming the clip if the table disagrees" is now "the table is the measurement", see Task 4's `push`; the death check is `check_death`).

**Placeholders.** None intended. The two places that say "if the printed axis differs, change the constant" (Task 4 Step 4) and "adjust until both figures are in frame" (Task 8 Step 4) are measurement steps with exact commands.

**Type consistency.** `MonsterClip::{Attack(u8), React(u8), Death}` used by `MonsterPlay`, `MonsterFade`, `pose`, `kobold_view`, `react`; `Mind::Attack { t, mv: u8 }` everywhere; `Sense.attack: Option<u8>`, `Sense.cooldown`; `AttackSpec` fields `name, length, hit, damage, poise, level, radius, radius2`; `Hit { level: u8, poise: f32, from: Vec3 }`; `SwingSpec { level: u8, poise: f32 }`; `dsclip::{checked_manifest, read_clip, cache_dir}`; `Rig.names: &DsNames`; `retarget::basis_for(.., rig)`; `retarget::ds_forward(.., feet)`. `combat::root_step` becomes `pub(crate)`.

---

## Deviations found while executing (2026-10-08)

Measured, then changed; the code and the spec follow these, the task text above is the plan as written.

- **Task 3.** The foot gate on the kobold: its feet are twice as long as the ghoul's scaled ones, so toe heights cannot be compared (0.144 yd gap on `a00_3000`, ankles within 0.02). `DsNames.soles` for the ghoul is its two ankles, `Rig.contact_height` is per rig (human 0.02, kobold 0.09), `wow_soles` [47, 48]. The cross check `Pelvis→Spine` (kobold bones 15 and 14 sit 12 mm apart) was noise (17° drift); it is the shoulder line `L UpperArm→R UpperArm` (36, 37) now. All 14 test clips pass 15°/15°/0.08 with margin (worst: direction 0.05°, cross 8.25°, foot 0.024 yd).
- **Task 4.** The ghoul's blade axis cannot be measured (the sword sweeps sideways; all six axes score ≈ 0 for "ahead", best 0.037): no `GHOUL_BLADE_AXIS`, no `blade_axis`. The hit volume runs from the right-hand attachment along the forearm (`reach <hand bone> <forearm bone>` = `53 45`) for `REACH_LENGTH`.
- **Task 5.** `a00_9000`-`9004` start lying (the feigned death), they are no death. The deaths are `a00_2202` (falls on its back, blow from the front) and `a00_2204` (falls on its face): `Kit.deaths`, manifest `death <clip> push <fwd> <left>`, `choose_death` in the game, `MonsterClip::Death(u8)`.
- **Task 7.** `Reach { hand, forearm }` (bone indices), `MonsterClips.deaths`.
- **Task 11.** `WOWSOULS_REACH` was not built (the constants are tuned against the trace; every forced move connects from the bands' edge). The kobold ran through the player (no body collision between them): a kit move's lunge stops `LUNGE_STOP_GAP` (0.3 yd, edge to edge) short of a player ahead of it. Pattern bands tuned from forced runs: `run_strike` (2.2, 3.4), `pounce` (2.2, 3.2). `in_strike_window` went away (the old strike reads the spec's window); `procedural_attack` pins the fallback swing (test).
- **Task 12.** Done inside Task 11's `react` (poise, deaths by push); `Poise` and `choose_react`/`choose_death` live in `monster.rs`, unit-tested.
- **Task 13.** Gates: fmt clean, clippy `-D warnings` clean, tests dsr-cache 26, dsr-formats 29, wow-souls 202, wscl 4 (install env set); WoW and DSR install listings identical before/after.
