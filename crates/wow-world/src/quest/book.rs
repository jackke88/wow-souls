//! `QuestBook`: an index over the [`World`]'s quests, built the way vMaNGOS's `ObjectMgr::LoadQuests`
//! builds its tables (report section 1.1): who gives and who takes each quest, the prerequisite lists,
//! the chain lists and the exclusive groups. It owns copies of the data, so the log and the rules take a
//! plain `&QuestBook`.

use super::{Class, Npc, Race};
use crate::model::{Quest, World};
use std::collections::{BTreeMap, BTreeSet};

pub struct QuestBook {
    quests: BTreeMap<u32, Quest>,
    /// The signed prerequisites of each quest (`prevQuests`): its own `PrevQuestId` (kept even when that
    /// quest is not in the book: the check then fails, as it does in the full database), and the
    /// quests that name it in `NextQuestId` (negative when that `NextQuestId` is negative).
    prev_quests: BTreeMap<u32, Vec<i32>>,
    /// `prevChainQuests`: the quests whose `NextQuestInChain` is this one.
    prev_chain: BTreeMap<u32, Vec<u32>>,
    /// The members of each non-zero `ExclusiveGroup`, by group (signed).
    groups: BTreeMap<i32, Vec<u32>>,
    givers: BTreeMap<u32, Vec<Npc>>,
    enders: BTreeMap<u32, Vec<Npc>>,
    gives: BTreeMap<Npc, Vec<u32>>,
    takes: BTreeMap<Npc, Vec<u32>>,
    /// The NPCs and objects that have a spawn in the valley (a creature's `alt` entries count).
    spawned: BTreeSet<Npc>,
    /// Quests started by an item (`item_extras.start_quest`) that drops in loot or is rewarded by a quest.
    item_started: BTreeSet<u32>,
    playable: BTreeSet<u32>,
    playable_by_class: BTreeMap<u8, BTreeSet<u32>>,
    creature_names: BTreeMap<u32, String>,
    item_names: BTreeMap<u32, String>,
}

impl QuestBook {
    pub fn new(world: &World) -> QuestBook {
        let quests: BTreeMap<u32, Quest> = world.quests.iter().map(|q| (q.id, q.clone())).collect();

        let mut prev_quests: BTreeMap<u32, Vec<i32>> = BTreeMap::new();
        let mut prev_chain: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        let mut groups: BTreeMap<i32, Vec<u32>> = BTreeMap::new();
        // In id order, as the loader walks the table.
        for q in quests.values() {
            if q.prev != 0 {
                prev_quests.entry(q.id).or_default().push(q.prev);
            }
            if q.next != 0 && quests.contains_key(&q.next.unsigned_abs()) {
                let signed = if q.next < 0 {
                    -(q.id as i32)
                } else {
                    q.id as i32
                };
                prev_quests
                    .entry(q.next.unsigned_abs())
                    .or_default()
                    .push(signed);
            }
            if q.next_in_chain != 0 && quests.contains_key(&q.next_in_chain) {
                prev_chain.entry(q.next_in_chain).or_default().push(q.id);
            }
            if q.exclusive_group != 0 {
                groups.entry(q.exclusive_group).or_default().push(q.id);
            }
        }

        let mut givers: BTreeMap<u32, Vec<Npc>> = BTreeMap::new();
        let mut gives: BTreeMap<Npc, Vec<u32>> = BTreeMap::new();
        for r in &world.starts {
            let npc = Npc::new(r.npc, r.object);
            givers.entry(r.quest).or_default().push(npc);
            gives.entry(npc).or_default().push(r.quest);
        }
        let mut enders: BTreeMap<u32, Vec<Npc>> = BTreeMap::new();
        let mut takes: BTreeMap<Npc, Vec<u32>> = BTreeMap::new();
        for r in &world.ends {
            let npc = Npc::new(r.npc, r.object);
            enders.entry(r.quest).or_default().push(npc);
            takes.entry(npc).or_default().push(r.quest);
        }
        for list in gives.values_mut().chain(takes.values_mut()) {
            list.sort_unstable();
            list.dedup();
        }

        let mut spawned: BTreeSet<Npc> = BTreeSet::new();
        for s in &world.spawns {
            spawned.insert(Npc::Creature(s.entry));
            spawned.extend(s.alt.iter().filter(|&&e| e != 0).map(|&e| Npc::Creature(e)));
        }
        spawned.extend(world.object_spawns.iter().map(|s| Npc::Object(s.entry)));

        let obtainable_items: BTreeSet<u32> = world
            .loot
            .iter()
            .filter(|l| l.min_or_ref >= 0)
            .map(|l| l.item)
            .chain(world.quests.iter().flat_map(|q| {
                q.reward_items
                    .iter()
                    .chain(&q.reward_choice)
                    .map(|&(id, _)| id)
            }))
            .collect();
        let item_started: BTreeSet<u32> = world
            .item_extras
            .iter()
            .filter(|ix| ix.start_quest != 0 && obtainable_items.contains(&ix.entry))
            .map(|ix| ix.start_quest)
            .collect();

        let mut book = QuestBook {
            quests,
            prev_quests,
            prev_chain,
            groups,
            givers,
            enders,
            gives,
            takes,
            spawned,
            item_started,
            playable: BTreeSet::new(),
            playable_by_class: BTreeMap::new(),
            creature_names: world
                .creatures
                .iter()
                .map(|c| (c.entry, c.name.clone()))
                .collect(),
            item_names: world
                .items
                .iter()
                .map(|i| (i.entry, i.name.clone()))
                .collect(),
        };
        for c in [
            Class::Warrior,
            Class::Paladin,
            Class::Hunter,
            Class::Rogue,
            Class::Priest,
            Class::Shaman,
            Class::Mage,
            Class::Warlock,
            Class::Druid,
        ] {
            book.playable_by_class
                .insert(c as u8, book.playable_set_for(c, Race::Human));
        }
        book.playable = book.playable_by_class[&(Class::Warrior as u8)].clone();
        book
    }

    pub fn quest(&self, id: u32) -> Option<&Quest> {
        self.quests.get(&id)
    }

    /// Every quest id in the book, ascending.
    pub fn ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.quests.keys().copied()
    }

    /// The signed prerequisite ids of a quest (see the field's note); empty = a first quest.
    pub fn prev_quests(&self, id: u32) -> &[i32] {
        self.prev_quests.get(&id).map_or(&[], Vec::as_slice)
    }

    /// The quests whose `NextQuestInChain` is `id`.
    pub fn prev_chain(&self, id: u32) -> &[u32] {
        self.prev_chain.get(&id).map_or(&[], Vec::as_slice)
    }

    /// The members of an `ExclusiveGroup` (signed as in the data), ascending.
    pub fn group(&self, group: i32) -> &[u32] {
        self.groups.get(&group).map_or(&[], Vec::as_slice)
    }

    /// The quests a creature (`object` false) or object gives, ascending.
    pub fn by_giver(&self, npc: u32, object: bool) -> &[u32] {
        self.gives
            .get(&Npc::new(npc, object))
            .map_or(&[], Vec::as_slice)
    }

    /// The quests a creature or object takes in (the ender), ascending.
    pub fn by_ender(&self, npc: u32, object: bool) -> &[u32] {
        self.takes
            .get(&Npc::new(npc, object))
            .map_or(&[], Vec::as_slice)
    }

    /// Who gives a quest (empty for the quests nobody offers).
    pub fn givers(&self, quest: u32) -> &[Npc] {
        self.givers.get(&quest).map_or(&[], Vec::as_slice)
    }

    /// Who takes a quest in.
    pub fn enders(&self, quest: u32) -> &[Npc] {
        self.enders.get(&quest).map_or(&[], Vec::as_slice)
    }

    /// Does the NPC or object have a spawn in the valley?
    pub fn is_spawned(&self, npc: Npc) -> bool {
        self.spawned.contains(&npc)
    }

    /// Some giver of the quest is spawned in the valley (a quest nobody gives is not).
    pub fn giver_spawned(&self, quest: u32) -> bool {
        self.givers(quest).iter().any(|&n| self.is_spawned(n))
    }

    /// Some ender of the quest is spawned in the valley: when none is, the quest can be accepted but
    /// not handed in (it is parked in the log: quests 54 and 2158 end in Goldshire).
    pub fn ender_spawned(&self, quest: u32) -> bool {
        self.enders(quest).iter().any(|&n| self.is_spawned(n))
    }

    /// Is the quest one the Human Warrior can ever be offered here: the class and race gates let the
    /// character in, a giver is spawned in the valley, and every prerequisite chain can be satisfied
    /// by playable quests. This leaves out the other classes' letters and trainer quests, the
    /// Stormwind chain 344-347 (givers outside the valley) and the quests nobody gives (5805, 7961, 7962):
    /// they stay in the data and never show.
    pub fn playable(&self, quest: u32) -> bool {
        self.playable.contains(&quest)
    }

    /// Whether `quest` is reachable for a Human of `class`.
    pub fn playable_for(&self, quest: u32, class: Class) -> bool {
        self.playable_by_class
            .get(&(class as u8))
            .is_some_and(|set| set.contains(&quest))
    }

    pub fn creature_name(&self, entry: u32) -> Option<&str> {
        self.creature_names.get(&entry).map(String::as_str)
    }

    pub fn item_name(&self, item: u32) -> Option<&str> {
        self.item_names.get(&item).map(String::as_str)
    }

    fn playable_set_for(&self, class: Class, race: Race) -> BTreeSet<u32> {
        let class = class.mask();
        let race = race.mask();
        let mut set: BTreeSet<u32> = self
            .quests
            .values()
            .filter(|q| {
                (q.required_classes == 0 || q.required_classes & class != 0)
                    && (q.required_races == 0 || q.required_races & race != 0)
                    && (self.giver_spawned(q.id) || self.item_started.contains(&q.id))
            })
            .map(|q| q.id)
            .collect();
        // Drop the quests whose prerequisites cannot be met by what is left, until nothing changes.
        loop {
            let unreachable: Vec<u32> = set
                .iter()
                .copied()
                .filter(|&id| !self.prerequisites_can_be_met(id, &set))
                .collect();
            if unreachable.is_empty() {
                return set;
            }
            for id in unreachable {
                set.remove(&id);
            }
        }
    }

    /// Could the prerequisites of `id` be met by quests of `reachable` (a quest with none: always)?
    /// Some prerequisite must be reachable, and when it is in a negative ("each from all") group so
    /// must be all the other members.
    fn prerequisites_can_be_met(&self, id: u32, reachable: &BTreeSet<u32>) -> bool {
        let prevs = self.prev_quests(id);
        prevs.is_empty()
            || prevs.iter().any(|&p| {
                let prev = p.unsigned_abs();
                let Some(q) = self.quests.get(&prev) else {
                    return false;
                };
                reachable.contains(&prev)
                    && (q.exclusive_group >= 0
                        || self
                            .group(q.exclusive_group)
                            .iter()
                            .all(|m| reachable.contains(m)))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Relation, Spawn};
    use crate::quest::testing::{quest, real_world};

    fn spawn(guid: u32, entry: u32, alt: [u32; 4]) -> Spawn {
        Spawn {
            guid,
            entry,
            alt,
            pos: [0.0; 3],
            orientation: 0.0,
            respawn_min: 0,
            respawn_max: 0,
            wander: 0.0,
            movement: 0,
            area: 9,
            event: false,
        }
    }

    fn rel(npc: u32, quest: u32) -> Relation {
        Relation {
            npc,
            quest,
            object: false,
        }
    }

    /// Quest ids with their (prev, next, next_in_chain, group), giver 100 + id spawned unless noted.
    fn world(rows: &[(u32, i32, i32, u32, i32)]) -> World {
        let mut w = World::default();
        for &(id, prev, next, chain, group) in rows {
            let mut q = quest(id);
            q.prev = prev;
            q.next = next;
            q.next_in_chain = chain;
            q.exclusive_group = group;
            w.quests.push(q);
            w.starts.push(rel(100 + id, id));
            w.ends.push(rel(200 + id, id));
            w.spawns.push(spawn(id, 100 + id, [0; 4]));
            w.spawns.push(spawn(1000 + id, 200 + id, [0; 4]));
        }
        w
    }

    #[test]
    fn next_quest_id_becomes_a_signed_prerequisite_of_its_target() {
        // 1 and 2 name 5 as their next quest (positive), 3 names it negative; 5 has a prev of its own (4).
        let book = QuestBook::new(&world(&[
            (1, 0, 5, 0, 0),
            (2, 0, 5, 0, 0),
            (3, 0, -5, 0, 0),
            (4, 0, 0, 0, 0),
            (5, 4, 0, 0, 0),
        ]));
        // In id order: the quests that name 5 as their next quest, then 5's own prev when 5 is walked.
        assert_eq!(book.prev_quests(5), &[1, 2, -3, 4]);
        assert_eq!(book.prev_quests(1), &[] as &[i32]);
    }

    #[test]
    fn a_missing_next_target_is_ignored_and_a_missing_prev_is_kept() {
        let mut w = world(&[(1, 0, 99, 0, 0), (2, 77, 0, 0, 0)]);
        w.quests[0].next_in_chain = 98;
        let book = QuestBook::new(&w);
        assert!(book.prev_quests(99).is_empty() && book.prev_chain(98).is_empty());
        // Quest 77 is not in the book: the prerequisite stays, and the rules fail it (the full database has it).
        assert_eq!(book.prev_quests(2), &[77]);
        assert!(!book.playable(2), "a prerequisite that cannot be met");
        assert!(book.playable(1));
    }

    #[test]
    fn the_chain_and_group_tables_list_their_members() {
        let book = QuestBook::new(&world(&[
            (1, 0, 0, 2, -7),
            (2, 0, 0, 3, -7),
            (3, 0, 0, 0, 4),
            (4, 0, 0, 0, 0),
        ]));
        assert_eq!(book.prev_chain(2), &[1]);
        assert_eq!(book.prev_chain(3), &[2]);
        assert!(book.prev_chain(1).is_empty());
        assert_eq!(book.group(-7), &[1, 2]);
        assert_eq!(book.group(4), &[3]);
        assert!(book.group(0).is_empty() && book.group(9).is_empty());
    }

    #[test]
    fn givers_enders_and_spawns_are_looked_up_both_ways() {
        let mut w = world(&[(1, 0, 0, 0, 0), (2, 0, 0, 0, 0)]);
        // Quest 2 is also given by an object, and its creature giver 102 is only an `alt` entry of a spawn.
        w.starts.push(Relation {
            npc: 7,
            quest: 2,
            object: true,
        });
        w.spawns.retain(|s| s.entry != 102);
        w.spawns.push(spawn(77, 5000, [0, 102, 0, 0]));
        let book = QuestBook::new(&w);
        assert_eq!(book.by_giver(101, false), &[1]);
        assert_eq!(book.by_giver(7, true), &[2]);
        assert!(
            book.by_giver(7, false).is_empty(),
            "an object is not a creature"
        );
        assert_eq!(book.by_ender(202, false), &[2]);
        assert_eq!(book.givers(2), &[Npc::Creature(102), Npc::Object(7)]);
        assert_eq!(book.enders(1), &[Npc::Creature(201)]);
        assert!(book.is_spawned(Npc::Creature(102)), "an alt entry counts");
        assert!(!book.is_spawned(Npc::Object(7)));
        assert!(book.giver_spawned(2) && book.ender_spawned(2));
        w.spawns.retain(|s| s.entry != 5000);
        let book = QuestBook::new(&w);
        assert!(
            !book.giver_spawned(2),
            "neither 102 nor the object is spawned now"
        );
        assert!(book.ender_spawned(2));
        assert!(!book.giver_spawned(999) && !book.ender_spawned(999));
    }

    #[test]
    fn playable_leaves_out_other_classes_races_orphans_and_what_hangs_on_them() {
        let mut w = world(&[
            (1, 0, 0, 0, 0),   // a first quest
            (2, 1, 0, 0, 0),   // follows 1
            (3, 0, 0, 0, 0),   // class gate: Priest only
            (4, 3, 0, 0, 0),   // follows the Priest quest
            (5, 0, 0, 0, 0),   // race gate: Orc or Dwarf only
            (6, 0, 0, 0, 0),   // nobody gives it
            (7, 0, 0, 0, 0),   // its giver is not spawned
            (8, 7, 0, 0, 0),   // follows 7
            (9, 0, 11, 0, -3), // 9 and 10 are an each-from-all pair that names 11 as the next quest
            (10, 0, 11, 0, -3),
            (11, 0, 0, 0, 0),
            (12, 0, 14, 0, -4), // like 9 / 10, but 13 (Rogue or Mage only) is the pair's other member
            (13, 0, 14, 0, -4),
            (14, 0, 0, 0, 0),
        ]);
        let set = |w: &mut World, id: u32, f: &dyn Fn(&mut Quest)| {
            f(w.quests.iter_mut().find(|q| q.id == id).unwrap());
        };
        set(&mut w, 3, &|q| q.required_classes = Class::Priest.mask());
        set(&mut w, 5, &|q| {
            q.required_races = Race::Orc.mask() | Race::Dwarf.mask()
        });
        set(&mut w, 13, &|q| {
            q.required_classes = Class::Rogue.mask() | Class::Mage.mask()
        });
        w.starts.retain(|r| r.quest != 6);
        w.spawns.retain(|s| s.entry != 107);
        let book = QuestBook::new(&w);
        let playable: Vec<u32> = book.ids().filter(|&id| book.playable(id)).collect();
        assert_eq!(
            playable,
            vec![1, 2, 9, 10, 11, 12],
            "14 needs 12 and 13 both (each from all) and 13 is a Rogue / Mage quest"
        );
        // A mask that lists the Warrior and the Human lets the quest in, and what follows it.
        set(&mut w, 3, &|q| {
            q.required_classes = Class::Priest.mask() | Class::Warrior.mask()
        });
        set(&mut w, 5, &|q| {
            q.required_races = Race::Orc.mask() | Race::Human.mask()
        });
        let book = QuestBook::new(&w);
        assert!(book.playable(3) && book.playable(4) && book.playable(5));
    }

    /// Report section 1.11: the derived `prevQuests` and `prevChainQuests` of the 28 quests.
    #[test]
    fn install_the_derived_prerequisite_lists_are_the_reports_table() {
        let Some(w) = real_world() else { return };
        let book = QuestBook::new(&w);
        let table: &[(u32, &[i32], &[u32])] = &[
            (6, &[18], &[]),
            (7, &[783], &[783]),
            (15, &[7], &[]),
            (18, &[783], &[]),
            (21, &[15], &[15]),
            (33, &[5261], &[5261]),
            (54, &[21], &[]),
            (344, &[343], &[]),
            (345, &[344], &[344]),
            (346, &[347], &[347]),
            (347, &[345], &[345]),
            (783, &[], &[]),
            (1598, &[], &[]),
            (2158, &[], &[]),
            (3100, &[7], &[]),
            (3101, &[7], &[]),
            (3102, &[7], &[]),
            (3103, &[7], &[]),
            (3104, &[7], &[]),
            (3105, &[7], &[]),
            (3903, &[18, 33], &[]),
            (3904, &[3903], &[3903]),
            (3905, &[3904], &[3904]),
            (5261, &[783], &[]),
            (5623, &[], &[]),
            (5805, &[], &[]),
            (7961, &[], &[]),
            (7962, &[], &[]),
        ];
        assert_eq!(book.ids().count(), 28);
        for &(id, prev, chain) in table {
            assert_eq!(book.prev_quests(id), prev, "prevQuests of {id}");
            assert_eq!(book.prev_chain(id), chain, "prevChainQuests of {id}");
        }
        // The exclusive group -18 holds exactly 18 and 33 (section 1.5).
        assert_eq!(book.group(-18), &[18, 33]);
    }

    /// Report section 2.0b: who gives and who takes what in the valley.
    #[test]
    fn install_givers_enders_and_the_quests_outside_the_valley() {
        let Some(w) = real_world() else { return };
        let book = QuestBook::new(&w);
        assert_eq!(
            book.by_giver(197, false),
            &[7, 15, 21, 54, 3100, 3101, 3102, 3103, 3104, 3105],
            "Marshal McBride"
        );
        assert_eq!(book.by_ender(197, false), &[7, 15, 21, 783]);
        assert_eq!(book.by_giver(823, false), &[6, 18, 783, 3903, 5261]);
        assert_eq!(book.by_ender(823, false), &[6, 18]);
        assert_eq!(book.by_giver(196, false), &[33]);
        assert_eq!(book.by_ender(196, false), &[33, 5261]);
        assert_eq!(book.by_giver(9296, false), &[3904, 3905]);
        assert_eq!(book.by_ender(9296, false), &[3903, 3904]);
        assert_eq!(book.by_ender(911, false), &[3100], "Llane Beshere");
        assert_eq!(book.by_ender(952, false), &[3905], "Brother Neals");
        // Marshal Dughan (54) and Innkeeper Farley (2158) have a template and no spawn.
        assert_eq!(book.enders(54), &[Npc::Creature(240)]);
        assert!(!book.ender_spawned(54) && !book.ender_spawned(2158));
        assert!(book.giver_spawned(54) && book.giver_spawned(2158));
        // The Stormwind chain and the orphans.
        assert!(!book.giver_spawned(344) && !book.giver_spawned(347));
        assert!(book.giver_spawned(345) && book.giver_spawned(346), "Paxton");
        for orphan in [5805, 7961, 7962] {
            assert!(book.givers(orphan).is_empty(), "{orphan} has no giver");
            assert!(!book.giver_spawned(orphan));
        }
        assert!(
            book.ender_spawned(5805),
            "Merissa Stilwell stands in the valley"
        );
        for q in [7, 15, 21, 783, 3100, 3903, 3904, 3905, 6, 18, 33, 5261] {
            assert!(book.ender_spawned(q), "{q} can be handed in");
        }
        assert_eq!(book.creature_name(197), Some("Marshal McBride"));
        assert_eq!(book.item_name(752), Some("Red Burlap Bandana"));
    }

    /// Report summary point 1: a Human Warrior can ever take exactly these 14 of the 28.
    #[test]
    fn install_exactly_the_fourteen_quests_of_the_report_are_playable() {
        let Some(w) = real_world() else { return };
        let book = QuestBook::new(&w);
        let playable: Vec<u32> = book.ids().filter(|&id| book.playable(id)).collect();
        assert_eq!(
            playable,
            vec![6, 7, 15, 18, 21, 33, 54, 783, 2158, 3100, 3903, 3904, 3905, 5261]
        );
        for not in [
            344, 345, 346, 347, 1598, 3101, 3102, 3103, 3104, 3105, 5623, 5805, 7961, 7962,
        ] {
            assert!(!book.playable(not), "{not}");
        }
    }

    #[test]
    fn install_elwynn_and_stormwind_quests_are_level_0_to_13_and_playable() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cache/world/elwynn.txt");
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let w = crate::manifest::parse(&text).unwrap();
        let book = QuestBook::new(&w);
        for id in book.ids() {
            let q = book.quest(id).unwrap();
            assert!(
                (0..=13).contains(&q.level) && q.min_level <= 13,
                "quest {id} ({}) has level={} min_level={}",
                q.title,
                q.level,
                q.min_level
            );
        }
        // Elwynn + Stormwind quests playable by a Human Warrior include Goldshire, Eastvale,
        // Westbrook, Stormwind City deliveries (61, 332, 333, 334, 6261, 6285), the Warrior
        // training chain (1638 -> 1639 -> 1640 -> 1665 -> 1666 -> 1667), and the item-started
        // Collector chain (123 -> 147).
        for q in [
            6, 7, 11, 15, 18, 21, 33, 35, 37, 39, 40, 45, 47, 52, 54, 59, 60, 61, 62, 71, 76, 83,
            84, 85, 86, 87, 88, 106, 107, 111, 112, 114, 123, 147, 176, 239, 332, 333, 334, 783,
            1638, 1639, 1640, 1665, 1666, 1667, 2158, 3100, 3903, 3904, 3905, 5261, 5545,
        ] {
            assert!(book.playable(q), "Warrior quest {q} should be playable");
        }
        // Class-specific chains in Northshire, Elwynn and Stormwind work for their classes.
        for q in [3104, 1860, 1861] {
            assert!(
                book.playable_for(q, Class::Mage),
                "Mage quest {q} should be playable"
            );
        }
        for q in [3101, 2998, 1641, 1642, 1643, 1644, 1780, 1781] {
            assert!(
                book.playable_for(q, Class::Paladin),
                "Paladin quest {q} should be playable"
            );
        }
        for q in [3102, 2205, 2206] {
            assert!(
                book.playable_for(q, Class::Rogue),
                "Rogue quest {q} should be playable"
            );
        }
        for q in [3103, 5623, 5624, 5635] {
            assert!(
                book.playable_for(q, Class::Priest),
                "Priest quest {q} should be playable"
            );
        }
        for q in [3105, 1598, 1685, 1688, 1689] {
            assert!(
                book.playable_for(q, Class::Warlock),
                "Warlock quest {q} should be playable"
            );
        }
    }
}
