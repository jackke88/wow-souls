use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn site(id: u32) -> Site {
    Site {
        id,
        name: format!("Graveyard {id}"),
        at: Vec3::ZERO,
        yaw: 0.0,
    }
}

fn directory() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "wow-souls-binding-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn soul_binding_save_is_strict() {
    assert_eq!(
        from_text(&to_text(87047, &[87047])).unwrap(),
        (87047, vec![87047])
    );
    assert_eq!(
        from_text("wssoulbind 1\nsite 5\n").unwrap(),
        (5, vec![5]),
        "the one-site save still reads"
    );
    assert_eq!(
        from_text(&to_text(2, &[1, 2, 3])).unwrap(),
        (2, vec![2, 1, 3])
    );
    for text in [
        "",
        "wssoulbind 2\nsite 1\n",
        "wssoulbind 1\nsite 0\n",
        "wssoulbind 1\nsite -1\n",
        "wssoulbind 1\nsite NaN\n",
        "wssoulbind 1\nsite 1 extra\n",
        "wssoulbind 1\nsite 1\nsite 2\n",
    ] {
        assert!(from_text(text).is_err(), "accepted {text:?}");
    }
}

#[test]
fn soul_binding_switches_persists_and_isolates_characters() {
    let dir = directory();
    let path = dir.join("one.txt");
    let mut one = Binding::read(path.clone(), vec![site(1), site(2)]);
    assert!(one.site().is_none());
    one.bind(1).unwrap();
    one.bind(2).unwrap();
    let restored = Binding::read(path, vec![site(1), site(2)]);
    assert_eq!(restored.site().unwrap().id, 2);
    assert_eq!(restored.site().unwrap().arrival(), Vec3::new(0.0, 0.0, 2.2));
    let two = Binding::read(dir.join("two.txt"), vec![site(1), site(2)]);
    assert!(two.site().is_none());
}

#[test]
fn soul_binding_preserves_corrupt_or_unknown_saves() {
    for text in ["corrupt save", "wssoulbind 1\nsite 77\n"] {
        let path = directory().join("soul.txt");
        std::fs::write(&path, text).unwrap();
        let mut state = Binding::read(path.clone(), vec![site(1)]);
        assert!(state.bind(1).is_err());
        assert!(state.bound.is_none());
        assert_eq!(std::fs::read_to_string(path).unwrap(), text);
    }
}

#[test]
fn soul_binding_does_not_change_when_the_disk_write_fails() {
    let dir = directory();
    let parent = dir.join("not-a-directory");
    std::fs::write(&parent, "occupied").unwrap();
    let mut state = Binding {
        path: parent.join("save.txt"),
        sites: vec![site(1), site(2)],
        bound: Some(1),
        known: vec![1],
        blocked: None,
    };
    assert!(state.bind(2).is_err());
    assert_eq!(state.bound, Some(1));
    assert_eq!(std::fs::read_to_string(parent).unwrap(), "occupied");
}

#[test]
fn soul_binding_requires_both_ground_range_and_height() {
    let sites = vec![site(1)];
    assert!(nearest(&sites, Vec3::new(0.0, 0.0, 2.2)).is_some());
    assert!(nearest(&sites, Vec3::new(0.0, 0.0, RANGE + 0.1)).is_none());
    assert!(nearest(&sites, Vec3::new(0.0, HEIGHT + 0.1, 0.0)).is_none());
    assert!(nearest(&sites, Vec3::splat(f32::NAN)).is_none());
}

#[test]
fn soul_binding_graveyards_are_database_healer_spawns() {
    use wow_world::model::Spawn;
    let mut world = World::default();
    world.areas.push((9, "Northshire Valley".into()));
    let spawn = Spawn {
        guid: 87047,
        entry: 6491,
        alt: [0; 4],
        pos: [-8946.23, -183.477, 79.9953],
        orientation: 0.0,
        respawn_min: 60,
        respawn_max: 60,
        wander: 0.0,
        movement: 0,
        area: 9,
        event: false,
    };
    world.spawns.push(spawn.clone());
    world.spawns.push(Spawn {
        guid: 2,
        event: true,
        ..spawn.clone()
    });
    world.spawns.push(Spawn {
        guid: 3,
        entry: 6774,
        ..spawn
    });
    let sites = sites(&world);
    assert_eq!(sites.len(), 1);
    assert_eq!(
        (sites[0].id, sites[0].name.as_str()),
        (87047, "Northshire Valley")
    );
    assert_eq!(sites[0].at, wow_to_bevy([-8946.23, -183.477, 79.9953]));
}

fn app() -> (App, Entity) {
    let mut app = App::new();
    app.insert_resource(Time::<Real>::default())
        .insert_resource(Binding::read(directory().join("soul.txt"), vec![site(1)]))
        .init_resource::<Toasts>()
        .add_message::<BindRequest>()
        .add_message::<SoulBound>()
        .add_systems(Update, apply);
    let mut motor = Motor::default();
    motor.grounded = true;
    let entity = app
        .world_mut()
        .spawn((
            Avatar,
            Transform::from_xyz(0.0, 0.0, 2.2),
            Action::default(),
            Health::full(100.0),
            motor,
        ))
        .id();
    (app, entity)
}

#[test]
fn soul_binding_interaction_saves_then_emits_success() {
    let (mut app, _) = app();
    app.world_mut().write_message(BindRequest(1));
    app.update();
    let state = app.world().resource::<Binding>();
    assert_eq!(state.bound, Some(1));
    assert_eq!(
        from_text(&std::fs::read_to_string(&state.path).unwrap()).unwrap(),
        (1, vec![1])
    );
    assert_eq!(app.world().resource::<Messages<SoulBound>>().len(), 1);
}

#[test]
fn soul_binding_refuses_combat_airborne_and_out_of_range_requests() {
    for scenario in ["combat", "airborne", "distant", "dead", "disk"] {
        let (mut app, avatar) = app();
        match scenario {
            "combat" => {
                app.world_mut()
                    .spawn((Hostile { alert: true }, Reaction::None));
            }
            "airborne" => app.world_mut().get_mut::<Motor>(avatar).unwrap().grounded = false,
            "distant" => {
                app.world_mut()
                    .get_mut::<Transform>(avatar)
                    .unwrap()
                    .translation
                    .x = 20.0
            }
            "dead" => app.world_mut().get_mut::<Health>(avatar).unwrap().cur = 0.0,
            "disk" => {
                let parent = directory().join("file");
                std::fs::write(&parent, "occupied").unwrap();
                app.world_mut().resource_mut::<Binding>().path = parent.join("soul.txt");
            }
            _ => unreachable!(),
        }
        app.world_mut().write_message(BindRequest(1));
        app.update();
        assert_eq!(app.world().resource::<Binding>().bound, None, "{scenario}");
        assert_eq!(
            app.world().resource::<Messages<SoulBound>>().len(),
            0,
            "{scenario}"
        );
    }
}

#[test]
fn death_returns_to_the_nearest_graveyard_bound_at() {
    let dir = directory();
    let far = |id: u32, x: f32| Site {
        at: Vec3::new(x, 0.0, 0.0),
        ..site(id)
    };
    let mut state = Binding::read(
        dir.join("near.txt"),
        vec![far(1, 0.0), far(2, 100.0), far(3, 200.0)],
    );
    state.bind(1).unwrap();
    state.bind(2).unwrap();
    // Graveyard 3 was never bound: dying beside it still goes to the nearest bound one.
    assert_eq!(
        state.respawn_site(Vec3::new(190.0, 0.0, 0.0)).unwrap().id,
        2
    );
    assert_eq!(state.respawn_site(Vec3::new(10.0, 0.0, 0.0)).unwrap().id, 1);
    let restored = Binding::read(
        dir.join("near.txt"),
        vec![far(1, 0.0), far(2, 100.0), far(3, 200.0)],
    );
    assert_eq!(restored.bound, Some(2));
    assert_eq!(restored.respawn_site(Vec3::ZERO).unwrap().id, 1);
}
