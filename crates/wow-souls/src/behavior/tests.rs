use super::*;

fn context(dist: f32) -> Context {
    Context {
        me: Vec3::ZERO,
        target: Vec3::new(0.0, 0.0, -dist),
        forward: Vec3::NEG_Z,
        facing: 1.0,
        reach: 2.8,
        near: 0.0,
        speed: 4.0,
        admitted: true,
        open: false,
        guarding: false,
        sight: true,
        can_attack: true,
        allies: 0,
        center: Vec3::ZERO,
        leader: None,
    }
}

#[test]
fn a_boar_must_aim_before_starting_a_charge_even_in_attack_range() {
    let mut state = State::default();
    let d = step(
        Role::BoarCharge,
        "stonetusk",
        &mut state,
        context(2.4),
        0.016,
    );
    assert!(!d.attack, "proximity must not bypass the boar's aim phase");
    assert_eq!(state.phase, Phase::Aim);
    assert_eq!(d.pace, 0.0);
}

fn tick(role: Role, variant: &str, state: &mut State, c: Context, seconds: f32) -> Decision {
    let mut d = Decision::hold(c);
    for _ in 0..(seconds / 0.05).ceil() as usize {
        d = step(role, variant, state, c, 0.05);
    }
    d
}

#[test]
fn all_47_runtime_bindings_match_the_approved_mapping() {
    let data = include_str!("../../../../docs/ENEMY-AI-MAPPING.tsv");
    let rows: Vec<_> = data
        .lines()
        .skip(1)
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .collect();
    assert_eq!(rows.len(), crate::species::ROSTER.len());
    let mut names = std::collections::BTreeSet::new();
    let mut roles = std::collections::BTreeSet::new();
    for row in rows {
        assert!(names.insert(row[1]));
        let def = crate::species::by_name(row[1]).expect("mapped species exists");
        assert_eq!(def.behavior.id(), row[2], "{}", def.name);
        roles.insert(row[2]);
    }
    assert_eq!(roles.len(), 15);
    assert_eq!(crate::species::HOGGER_TAURUS.behavior, Role::BossCommitted);
}

#[test]
fn charge_pressure_and_ambush_have_different_decision_sequences() {
    let c = Context {
        near: 2.0,
        ..context(1.8)
    };
    let mut boar = State::default();
    let mut bear = State::default();
    let mut spider = State::default();
    assert!(!step(Role::BoarCharge, "stonetusk", &mut boar, c, 0.05).attack);
    assert!(step(Role::BearPressure, "bear", &mut bear, c, 0.05).attack);
    assert!(!step(Role::SpiderAmbush, "forest_spider", &mut spider, c, 0.05).attack);
    assert!(tick(Role::BoarCharge, "stonetusk", &mut boar, c, 0.7).attack);
    assert!(!tick(Role::SpiderAmbush, "forest_spider", &mut spider, c, 0.7).attack);
    assert!(tick(Role::SpiderAmbush, "forest_spider", &mut spider, c, 0.2).attack);
    boar.finished(Role::BoarCharge, "stonetusk");
    assert_eq!(boar.phase, Phase::Recover);
    assert!(!tick(Role::BoarCharge, "stonetusk", &mut boar, c, 0.3).attack);
}

#[test]
fn boars_keep_aiming_when_the_player_moves_behind_them() {
    let mut s = State::default();
    let c = Context {
        facing: 0.1,
        ..context(2.0)
    };
    assert!(!tick(Role::BoarCharge, "rockhide", &mut s, c, 2.0).attack);
    assert_eq!(s.phase, Phase::Aim);
    assert!(
        step(
            Role::BoarCharge,
            "rockhide",
            &mut s,
            Context { facing: 1.0, ..c },
            0.05
        )
        .attack
    );
}

#[test]
fn pressure_and_ambush_roles_never_choose_an_orbit() {
    for role in [
        Role::BoarCharge,
        Role::BearPressure,
        Role::GnollPressure,
        Role::SpiderAmbush,
        Role::HeavyRaider,
        Role::DefiasBrawler,
    ] {
        let mut s = State::default();
        let c = context(1.8);
        for _ in 0..80 {
            let d = step(role, "test", &mut s, c, 0.05);
            assert_eq!(d.goal.x, 0.0, "{role:?}");
            assert_ne!(s.phase, Phase::Flank);
            if d.attack {
                s.finished(role, "test");
            }
        }
    }
}

#[test]
fn a_recovering_bear_follows_but_a_gnoll_holds_its_line() {
    let c = Context {
        near: 2.0,
        ..context(4.0)
    };
    let mut bear = State::default();
    bear.finished(Role::BearPressure, "bear");
    let mut gnoll = State::default();
    gnoll.finished(Role::GnollPressure, "outrunner");
    assert!(step(Role::BearPressure, "bear", &mut bear, c, 0.05).pace > 0.0);
    assert_eq!(
        step(Role::GnollPressure, "outrunner", &mut gnoll, c, 0.05).pace,
        0.0
    );
    let mut group = State::default();
    let d = step(
        Role::GnollPressure,
        "outrunner",
        &mut group,
        Context { allies: 2, ..c },
        0.05,
    );
    assert_ne!(
        d.goal.x, 0.0,
        "a warband advances on separate front positions"
    );
    assert_eq!(group.phase, Phase::Approach);
}

#[test]
fn a_kobold_repositions_to_a_fixed_point_after_a_sortie() {
    let mut s = State::default();
    s.seed(19);
    s.finished(Role::KoboldSkirmisher, "miner");
    let a = step(Role::KoboldSkirmisher, "miner", &mut s, context(1.0), 0.05);
    let b = step(
        Role::KoboldSkirmisher,
        "miner",
        &mut s,
        Context {
            me: Vec3::X * 0.2,
            ..context(1.0)
        },
        0.05,
    );
    assert_eq!(
        a.goal, b.goal,
        "the destination must not run away every frame"
    );
    assert!(a.goal.z > 0.0);
    assert!(!a.attack);
    assert_ne!(
        tick(Role::KoboldSkirmisher, "miner", &mut s, context(1.0), 1.1).goal,
        a.goal
    );
}

#[test]
fn heavy_kobolds_hold_ground_instead_of_retreating() {
    for name in ["worker", "ruklar", "narg"] {
        let mut s = State::default();
        s.finished(Role::KoboldSkirmisher, name);
        assert_eq!(s.phase, Phase::Recover);
    }
}

#[test]
fn wolves_flank_but_murlocs_gather_before_their_rush() {
    let c = Context {
        allies: 2,
        center: Vec3::new(0.0, 0.0, 1.0),
        ..context(4.0)
    };
    let mut wolf = State::default();
    let mut murloc = State::default();
    let w = step(Role::WolfPack, "wolf", &mut wolf, c, 0.05);
    let m = step(Role::MurlocSwarm, "murloc", &mut murloc, c, 0.05);
    assert_eq!(wolf.phase, Phase::Flank);
    assert_ne!(w.goal.x, 0.0);
    assert_eq!(murloc.phase, Phase::Gather);
    assert_eq!(m.goal, c.center);
    tick(Role::MurlocSwarm, "murloc", &mut murloc, c, 0.8);
    assert_eq!(murloc.phase, Phase::Approach);
}

#[test]
fn a_gap_between_bite_and_lunge_makes_the_creature_close_in() {
    let mut s = State {
        phase: Phase::Engage,
        ..Default::default()
    };
    let c = Context {
        near: 1.3,
        reach: 2.4,
        can_attack: false,
        ..context(1.45)
    };
    let d = step(Role::WolfPack, "wolf", &mut s, c, 0.05);
    assert_eq!(s.phase, Phase::Approach);
    assert!(d.pace > 0.0);
    assert!(!d.attack);
}

#[test]
fn waiting_pack_members_keep_one_reachable_position_across_an_attack_band_gap() {
    for role in [Role::WolfPack, Role::KoboldSkirmisher, Role::DefiasRogue] {
        let mut s = State {
            phase: Phase::Engage,
            ..Default::default()
        };
        let c = Context {
            near: 1.3,
            reach: 2.4,
            admitted: false,
            can_attack: false,
            ..context(1.45)
        };
        let d = step(role, "test", &mut s, c, 0.05);
        assert_eq!(s.phase, Phase::Engage);
        assert!(!d.attack);
        assert!(flat(d.goal - c.target).length() < c.reach);
        let moved = Context { me: d.goal, ..c };
        for _ in 0..30 {
            let waiting = step(role, "test", &mut s, moved, 0.05);
            assert_eq!(s.phase, Phase::Engage);
            assert_eq!(waiting.goal, d.goal);
            assert!(!waiting.attack);
        }
    }
}

#[test]
fn a_caster_gets_one_finite_escape_when_pressed() {
    let mut s = State::default();
    let c = Context {
        near: 2.0,
        reach: 14.0,
        ..context(1.0)
    };
    let first = step(Role::CasterKite, "rogue_wizard", &mut s, c, 0.05);
    assert_eq!(s.phase, Phase::Withdraw);
    assert!(!first.attack);
    let last = tick(Role::CasterKite, "rogue_wizard", &mut s, c, 1.2);
    assert!(
        last.attack,
        "a player who follows the escape can catch and fight the caster"
    );
    assert_eq!(s.phase, Phase::Engage);
}

#[test]
fn a_caster_keeps_its_casting_turn_when_the_player_backs_across_the_hold_distance() {
    let mut s = State {
        phase: Phase::Engage,
        escaped: true,
        ..Default::default()
    };
    let at = |dist| Context {
        near: 1.3,
        reach: 14.0,
        ..context(dist)
    };
    for dist in [7.1, 6.98, 7.02, 7.0, 7.1, 7.4] {
        assert!(step(Role::CasterKite, "geomancer", &mut s, at(dist), 0.016).attack);
        assert_eq!(s.phase, Phase::Engage);
    }
    assert!(!step(Role::CasterKite, "geomancer", &mut s, at(7.6), 0.016).attack);
    assert_eq!(s.phase, Phase::Approach);
    assert!(!step(Role::CasterKite, "geomancer", &mut s, at(7.0), 0.016).attack);
    assert_eq!(s.phase, Phase::Approach);
    assert!(step(Role::CasterKite, "geomancer", &mut s, at(6.4), 0.016).attack);
    assert_eq!(s.phase, Phase::Engage);
}

#[test]
fn a_caster_with_blocked_sight_never_starts_a_bolt() {
    let c = Context {
        sight: false,
        near: 2.0,
        reach: 14.0,
        ..context(6.0)
    };
    let mut s = State::default();
    for _ in 0..80 {
        assert!(!step(Role::CasterKite, "surena", &mut s, c, 0.05).attack);
    }
    assert_eq!(s.phase, Phase::Approach);
}

#[test]
fn leader_and_escort_choose_opposite_sides_of_the_protection_line() {
    let c = Context {
        me: Vec3::ZERO,
        target: Vec3::X * 8.0,
        center: Vec3::X * 3.0,
        allies: 3,
        leader: Some(Vec3::ZERO),
        ..context(8.0)
    };
    let leader = step(
        Role::ProtectedLeader,
        "dockmaster",
        &mut State::default(),
        c,
        0.05,
    );
    let guard = step(
        Role::LeaderEscort,
        "bodyguard",
        &mut State::default(),
        c,
        0.05,
    );
    assert!(leader.goal.x < c.center.x);
    assert!(guard.goal.x > 0.0);
    assert!(!leader.attack);
    assert!(!guard.attack);
}

#[test]
fn a_leader_without_bodyguards_returns_to_close_combat() {
    let mut s = State {
        phase: Phase::Protect,
        ..Default::default()
    };
    let c = Context {
        near: 1.4,
        ..context(1.0)
    };
    assert!(step(Role::ProtectedLeader, "dockmaster", &mut s, c, 0.05).attack);
}

#[test]
fn rogues_observe_an_opening_before_changing_their_plan() {
    let c = Context {
        open: true,
        ..context(2.0)
    };
    let mut s = State::default();
    step(Role::DefiasRogue, "cutpurse", &mut s, c, 0.016);
    assert_eq!(s.phase, Phase::Flank);
    tick(Role::DefiasRogue, "cutpurse", &mut s, c, 0.1);
    assert_eq!(s.phase, Phase::Flank);
    tick(Role::DefiasRogue, "cutpurse", &mut s, c, 0.2);
    assert_eq!(s.phase, Phase::Engage);
}

#[test]
fn reset_discards_waypoints_openings_and_attack_priority() {
    let mut s = State::default();
    s.seed(17);
    tick(Role::WolfPack, "wolf", &mut s, context(4.0), 0.2);
    assert!(s.point.is_some());
    s.reset();
    assert_eq!(s.phase, Phase::Assess);
    assert_eq!(s.wait, 0.0);
    assert!(s.point.is_none());
    assert_eq!(s.open_for, 0.0);
}

fn actor(id: u64, role: Role, wait: f32) -> Actor {
    Actor {
        id,
        role,
        wait,
        at: Vec3::ZERO,
        attacking: false,
        ready: true,
    }
}

#[test]
fn local_admission_is_fair_and_independent_of_query_order() {
    let mut actors: Vec<_> = (0..6)
        .map(|id| actor(id, Role::KoboldSkirmisher, 1.0))
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..6 {
        let expected = admissions(&actors, Vec3::ZERO);
        actors.reverse();
        assert_eq!(expected, admissions(&actors, Vec3::ZERO));
        assert_eq!(expected.len(), 2);
        for a in &mut actors {
            if expected.contains(&a.id) {
                seen.insert(a.id);
                a.wait = 0.0;
            } else {
                a.wait += 1.0;
            }
        }
    }
    assert_eq!(seen.len(), 6);
}

#[test]
fn a_wolf_pack_has_one_hunter_and_distant_fights_do_not_take_slots() {
    let mut actors = vec![
        actor(1, Role::WolfPack, 3.0),
        actor(2, Role::WolfPack, 2.0),
        actor(3, Role::MurlocSwarm, 1.0),
    ];
    assert_eq!(admissions(&actors, Vec3::ZERO), vec![1, 3]);
    let mut far = actor(4, Role::BearPressure, 10.0);
    far.at = Vec3::X * 100.0;
    far.attacking = true;
    actors.push(far);
    assert_eq!(admissions(&actors, Vec3::ZERO), vec![1, 3]);
    actors[0].attacking = true;
    actors[0].ready = false;
    assert_eq!(admissions(&actors, Vec3::ZERO), vec![3]);
}

#[test]
fn attack_preferences_only_select_valid_openers_and_existing_project_kit_moves() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../anim/source");
    let roster = crate::monster::load(&root).expect("project kit library");
    for def in crate::species::ROSTER {
        let kit = roster.get(def.kit).expect(def.kit);
        for tenth in 0..160 {
            let c = context(tenth as f32 / 10.0);
            if let Some(row) = preferred(def.behavior, &State::default(), c, def.table) {
                let row = &def.table[row];
                assert!(row.weight > 0);
                assert!(
                    kit.attack_index(row.name).is_some(),
                    "{} {}",
                    def.name,
                    row.name
                );
                assert!(c.distance() >= row.band.0 && c.distance() <= row.band.1);
            }
        }
    }
    let c = Context {
        guarding: true,
        ..context(0.8)
    };
    assert_eq!(
        crate::pattern::BANDIT[preferred(
            Role::DefiasBrawler,
            &State::default(),
            c,
            crate::pattern::BANDIT
        )
        .unwrap()]
        .name,
        "kick"
    );
    assert!(preferred(
        Role::DefiasBrawler,
        &State::default(),
        c,
        crate::pattern::THUG
    )
    .is_none());
}

#[test]
fn terrain_probes_stop_a_retreat_at_a_cliff_and_permit_resident_ground() {
    use bevy::ecs::system::RunSystemOnce;
    let mut app = crate::motor::testkit::world_from_profile(&[(-10.0, 0.0), (0.0, 0.0)]);
    let edge = app
        .world_mut()
        .run_system_once(|w: benilla_world::collision::WorldCollision| {
            safe_direction(&w, Vec3::new(-0.1, 0.0, 0.0), Vec3::X, 0.3)
        })
        .unwrap();
    assert_eq!(edge, Vec3::ZERO);
    let good = app
        .world_mut()
        .run_system_once(|w: benilla_world::collision::WorldCollision| {
            safe_direction(&w, Vec3::new(-2.0, 0.0, 0.0), Vec3::NEG_X, 0.3)
        })
        .unwrap();
    assert_eq!(good, Vec3::NEG_X);
}

fn npc_world(def: &'static crate::species::SpeciesDef, brain: crate::npc::Brain) -> (App, Entity) {
    use crate::npc::*;
    let mut app = crate::motor::testkit::world_from_profile(&[(-10.0, 0.0), (10.0, 0.0)]);
    app.insert_resource(crate::monster::FoeRng(crate::pattern::Rng(41)))
        .init_resource::<crate::gear_effect::GearFx>()
        .add_message::<PlayerHurt>()
        .add_message::<Telegraph>()
        .add_message::<crate::foebolt::CastTell>();
    let hurtbox = crate::hits::Hurtbox {
        radius: 0.3,
        height: 1.8,
    };
    app.world_mut().spawn((
        crate::avatar::Avatar,
        Transform::from_xyz(0.0, 0.0, -1.0),
        crate::combat::Action::default(),
        crate::combat::Health::full(290.0),
        hurtbox,
    ));
    let anims = benilla_assets::ModelAnimations {
        graph: Handle::default(),
        clips: vec![],
        hand_close: [None, None],
        playable_animation_lookup: vec![],
        animation_lookup: vec![],
        global_bones: vec![],
        first_seq: None,
        pose: Default::default(),
    };
    let npc = app
        .world_mut()
        .spawn((
            crate::species::Species::new(def),
            Transform::default(),
            Hostile::default(),
            brain,
            Reaction::None,
            Home {
                feet: Vec3::ZERO,
                facing: 0.0,
                lockable: crate::lockon::Lockable {
                    aim_height: 0.8,
                    height: 1.2,
                },
                hurtbox,
                body: (0.3, 1.2),
            },
            crate::motor::MoveIntent::default(),
            crate::monster::MonsterPlay::default(),
            anims,
            Roam::new(crate::roam::Area::Stand, 41),
            crate::boss::BossPhase::default(),
            crate::boss::Tempo(1.0),
        ))
        .id();
    (app, npc)
}

#[test]
fn the_real_npc_loop_requires_sight_or_audible_movement() {
    use bevy::ecs::system::RunSystemOnce;
    // NPC faces -Z. Positive player Z approaches its back.
    for (z, speed, expected) in [
        (-12.0, 0.0, true),
        (6.0, 0.0, false),
        (4.0, 2.5, false),
        (2.4, 2.5, true),
        (5.0, 4.2, true),
        (8.0, 4.2, false),
        (9.0, 7.0, true),
        (11.0, 7.0, false),
    ] {
        let (mut app, npc) = npc_world(&crate::species::KOBOLD, Default::default());
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::avatar::Avatar>>()
            .single(app.world())
            .unwrap();
        let mut motor = crate::motor::Motor::default();
        motor.velocity = Vec3::Z * speed;
        motor.grounded = true;
        app.world_mut()
            .entity_mut(player)
            .insert((Transform::from_xyz(0.0, 0.0, z), motor));
        app.world_mut().run_system_once(crate::npc::think).unwrap();
        assert_eq!(
            app.world().get::<crate::npc::Hostile>(npc).unwrap().alert,
            expected,
            "player z={z}, speed={speed}"
        );
    }
}

#[test]
fn the_real_npc_loop_hears_a_shout_behind_it_but_not_through_a_wall() {
    use avian3d::prelude::{Collider, RigidBody};
    use bevy::ecs::system::RunSystemOnce;
    for blocked in [false, true] {
        let (mut app, npc) = npc_world(&crate::species::KOBOLD, Default::default());
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::avatar::Avatar>>()
            .single(app.world())
            .unwrap();
        app.world_mut().entity_mut(player).insert((
            Transform::from_xyz(0.0, 0.0, 12.0),
            crate::abilities::moves::UpperCast::new(crate::abilities::moves::ANIM_ROAR, 1.0),
        ));
        if blocked {
            app.world_mut().spawn((
                RigidBody::Static,
                Collider::cuboid(8.0, 6.0, 0.5),
                Transform::from_xyz(0.0, 2.0, 6.0),
            ));
            // The fixture's initial update seats colliders; finish seats Avian diagnostics
            // before a second update actually steps its physics schedule.
            app.finish();
            app.cleanup();
            app.update();
        }
        app.world_mut().run_system_once(crate::npc::think).unwrap();
        assert_eq!(
            app.world().get::<crate::npc::Hostile>(npc).unwrap().alert,
            !blocked,
            "wall={blocked}"
        );
    }
}

#[test]
fn the_real_npc_loop_preserves_aim_and_eventually_admits_the_attack() {
    use bevy::ecs::system::RunSystemOnce;
    let brain = crate::npc::Brain {
        mind: crate::npc::Mind::Chase,
        ..Default::default()
    };
    let (mut app, npc) = npc_world(&crate::species::STONETUSK, brain);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs_f32(0.05));
    app.world_mut().run_system_once(crate::npc::think).unwrap();
    let brain = app.world().get::<crate::npc::Brain>(npc).unwrap();
    assert_eq!(brain.mind, crate::npc::Mind::Chase);
    assert_eq!(brain.tactic.phase, Phase::Aim);
    let mut started = false;
    for _ in 0..30 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.05));
        app.world_mut().run_system_once(crate::npc::think).unwrap();
        if matches!(
            app.world().get::<crate::npc::Brain>(npc).unwrap().mind,
            crate::npc::Mind::Attack { .. }
        ) {
            started = true;
            break;
        }
    }
    assert!(
        started,
        "attack admission must not deadlock the actual brain"
    );
}

#[test]
fn the_real_npc_fallback_queues_at_its_own_range_when_no_kit_is_loaded() {
    use bevy::ecs::system::RunSystemOnce;
    let mut brain = crate::npc::Brain {
        mind: crate::npc::Mind::Chase,
        ..Default::default()
    };
    brain.tactic.phase = Phase::Engage;
    let (mut app, npc) = npc_world(&crate::species::MINER, brain);
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<crate::avatar::Avatar>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation
        .z = -1.8;
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs_f32(0.05));
    app.world_mut().run_system_once(crate::npc::think).unwrap();
    assert!(matches!(
        app.world().get::<crate::npc::Brain>(npc).unwrap().mind,
        crate::npc::Mind::Attack { .. }
    ));
}

#[test]
fn a_cornered_caster_can_cross_the_gap_to_its_real_melee_band() {
    use bevy::ecs::system::RunSystemOnce;
    let mut brain = crate::npc::Brain {
        mind: crate::npc::Mind::Chase,
        ..Default::default()
    };
    brain.tactic.phase = Phase::Engage;
    brain.tactic.escaped = true;
    let (mut app, npc) = npc_world(&crate::species::ROGUE_WIZARD, brain);
    let library = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../anim/source");
    app.insert_resource(crate::monster::load(&library).unwrap());
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<crate::avatar::Avatar>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation
        .z = -2.2;
    for _ in 0..2 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.05));
        app.world_mut().run_system_once(crate::npc::think).unwrap();
    }
    let intent = app.world().get::<crate::motor::MoveIntent>(npc).unwrap();
    assert!(intent.speed > 0.0 && intent.dir.z < 0.0, "{intent:?}");
}

#[test]
fn the_real_npc_loop_moves_a_reposition_during_the_existing_cooldown() {
    use bevy::ecs::system::RunSystemOnce;
    let mut brain = crate::npc::Brain {
        mind: crate::npc::Mind::Cooldown { left: 1.0 },
        ..Default::default()
    };
    brain.tactic.finished(Role::KoboldSkirmisher, "miner");
    let (mut app, npc) = npc_world(&crate::species::MINER, brain);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs_f32(0.05));
    app.world_mut().run_system_once(crate::npc::think).unwrap();
    let intent = app.world().get::<crate::motor::MoveIntent>(npc).unwrap();
    assert!(intent.speed > 0.0 && intent.dir.z > 0.0, "{intent:?}");
    assert!(matches!(
        app.world().get::<crate::npc::Brain>(npc).unwrap().mind,
        crate::npc::Mind::Cooldown { .. }
    ));
}

#[test]
fn a_boar_out_of_reach_sprints_in_and_strikes_again_soon() {
    let mut s = State::default();
    let far = step(Role::BoarCharge, "stonetusk", &mut s, context(8.0), 0.05);
    assert!(far.pace > 1.0, "a boar rushes in: pace {}", far.pace);
    let c = Context {
        near: 1.4,
        ..context(1.0)
    };
    let mut s = State::default();
    // The first step enters Aim; allow one 50 ms tick past its 0.25 s duration
    // rather than requiring a floating-point timer to hit exactly zero.
    assert!(tick(Role::BoarCharge, "stonetusk", &mut s, c, 0.35).attack);
    s.finished(Role::BoarCharge, "stonetusk");
    assert!(
        tick(Role::BoarCharge, "stonetusk", &mut s, c, 0.65).attack,
        "recover and aim take well under a second"
    );
}
