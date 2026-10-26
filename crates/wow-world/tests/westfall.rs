use std::path::Path;
use wow_world::{manifest, merge, model::World, select::spawns_without_path};
fn pack(zone: &str) -> World {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/world");
    manifest::parse(&std::fs::read_to_string(root.join(format!("{zone}.txt"))).unwrap()).unwrap()
}
#[test]
fn westfall_and_existing_guids_survive_the_packaged_merge() {
    let elwynn = pack("elwynn");
    let westfall = pack("westfall");
    let combined = pack("elwynn-westfall");
    assert_eq!(westfall.spawns.len(), 1245);
    assert_eq!(westfall.creatures.len(), 111);
    assert_eq!(westfall.quests.len(), 48);
    assert!(westfall.quests.iter().any(|q| q.level > 13));
    assert!(spawns_without_path(&westfall).is_empty());
    assert_eq!(
        merge::worlds(elwynn.clone(), westfall.clone()).unwrap(),
        combined
    );
    for original in [&elwynn, &westfall] {
        for spawn in &original.spawns {
            assert!(combined.spawns.contains(spawn));
        }
        for quest in &original.quests {
            assert!(combined.quests.contains(quest));
        }
        for waypoint in &original.waypoints {
            assert!(combined.waypoints.contains(waypoint));
        }
    }
    for relation in westfall.starts.iter().chain(&westfall.ends) {
        assert!(westfall.quests.iter().any(|q| q.id == relation.quest));
        if relation.object {
            assert!(westfall.objects.iter().any(|o| o.entry == relation.npc));
        } else {
            assert!(westfall.creatures.iter().any(|c| c.entry == relation.npc));
        }
    }
}
