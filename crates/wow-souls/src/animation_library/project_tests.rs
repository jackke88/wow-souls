//! Mandatory checks use project files, not optional installed-source/cache tests.
use std::path::{Path, PathBuf};

fn library() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../anim/source")
}

#[test]
fn every_inventory_clip_is_project_text_with_its_original_shape() {
    let root = library();
    let inventory =
        std::fs::read_to_string(root.join("inventory.tsv")).expect("project animation inventory");
    let mut count = 0;
    for row in inventory.lines().filter(|l| !l.starts_with('#')) {
        let fields: Vec<_> = row.split('\t').collect();
        assert_eq!(fields.len(), 6, "{row}");
        let path = root.join(fields[1]);
        let clip = super::read(&path).unwrap();
        assert_eq!(
            clip.frames.len(),
            fields[2].parse::<usize>().unwrap(),
            "{}",
            path.display()
        );
        assert_eq!(clip.bones(), fields[3].parse::<usize>().unwrap());
        assert_eq!(clip.fps, fields[4].parse::<f32>().unwrap());
        assert_eq!(clip.root_motion.len(), clip.frames.len());
        count += 1;
    }
    assert!(count >= 1000, "the active kits must all be present");
}

#[test]
fn all_gameplay_animation_loaders_work_from_project_files_without_binary_cache() {
    let root = library();
    let text = crate::motion_clip::checked_manifest(&root).unwrap();
    assert!(!text.lines().any(|l| l.starts_with("source ")));
    assert!(!root.join("human_male/body_roll.wscl").exists());
    let roll = crate::motion_clip::read_clip(&root, "human_male", "body_roll").unwrap();
    crate::motion_clip::load_moves(&root, &text, roll.bones()).unwrap();
    crate::locomotion::load(&root, roll.bones()).unwrap();
    let roster = crate::monster::load(&root).unwrap();
    assert!(roster.get("wolf").is_some() && roster.get("ambusher").is_some());
    crate::reaction::load(&root).unwrap();
    assert!(crate::defense::load(&text, &root).unwrap().is_some());
    crate::loot::interaction::Kit::load(&root).unwrap();
    crate::spells::cast::Kit::load(&root).unwrap();
    let index = std::fs::read_to_string(root.join("weapons.txt")).unwrap();
    let mut families = 0;
    for row in index.lines().skip(1) {
        let fields: Vec<_> = row.split_whitespace().collect();
        let ["family", _, path] = fields[..] else {
            panic!("{row}")
        };
        let dir = root.join(path);
        let manifest = crate::motion_clip::checked_manifest(&dir).unwrap();
        assert!(!manifest.lines().any(|l| l.starts_with("source ")));
        crate::motion_clip::load_moves(&dir, &manifest, roll.bones()).unwrap();
        families += 1;
    }
    assert_eq!(families, 19);
    // These names collide, but belong to different rigs. They must never resolve to the player.
    let wolf = crate::motion_clip::read_clip(&root, "wolf", "body_attack_01").unwrap();
    let kobold = crate::motion_clip::read_clip(&root, "kobold", "body_attack_01").unwrap();
    assert_ne!(wolf.bones(), kobold.bones());
}

#[test]
fn a_missing_or_broken_project_clip_never_falls_back_to_a_binary_of_the_same_name() {
    let dir = std::env::temp_dir().join(format!("wow-project-animation-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("wolf")).unwrap();
    let clip = wscl::Clip {
        name: "body_attack_01".into(),
        fps: 30.0,
        parents: vec![-1],
        driven: vec![true],
        frames: vec![vec![wscl::Trs::IDENTITY]; 2],
        root_motion: vec![[0.0; 4]; 2],
        ..Default::default()
    };
    clip.write(&mut std::fs::File::create(dir.join("wolf/body_attack_01.wscl")).unwrap())
        .unwrap();
    let path = dir.join("wolf/body_attack_01.wsanim");
    std::fs::write(&path, "not an animation").unwrap();
    let err = crate::motion_clip::read_clip(&dir, "wolf", "body_attack_01").unwrap_err();
    assert!(err.contains(".wsanim"));
    std::fs::remove_file(&path).unwrap();
    let err = crate::motion_clip::read_clip(&dir, "wolf", "body_attack_01").unwrap_err();
    assert!(err.contains(".wsanim"), "{err}");
    std::fs::remove_file(dir.join("wolf/body_attack_01.wscl")).unwrap();
    std::fs::remove_dir(dir.join("wolf")).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn extraction_manifests_are_rejected_without_reading_their_source_paths() {
    let dir = std::env::temp_dir().join(format!("wow-project-manifest-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for source in [
        "source 0 0 Z:/absent-source/chr/human_reference.anibnd.dcx",
        "source",
    ] {
        std::fs::write(
            dir.join("manifest.txt"),
            format!(
                "wscl-cache 6\nextractor {}\n{source}\n",
                wscl::EXTRACTOR_VERSION
            ),
        )
        .unwrap();
        let err = crate::motion_clip::checked_manifest(&dir).unwrap_err();
        assert!(
            err.starts_with("extraction manifests are unsupported"),
            "{err}"
        );
    }
    std::fs::remove_file(dir.join("manifest.txt")).unwrap();
    std::fs::remove_dir(dir).unwrap();
}
