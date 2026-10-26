//! Combine imports from the same database snapshot without duplicating shared quest dependencies.
use crate::model::World;
use anyhow::{bail, Result};

fn union<T: PartialEq>(into: &mut Vec<T>, from: Vec<T>) {
    for row in from {
        if !into.contains(&row) {
            into.push(row);
        }
    }
}

pub fn worlds(mut left: World, right: World) -> Result<World> {
    if left.xp_for_level != right.xp_for_level {
        bail!("world imports have different XP tables; use the same database snapshot");
    }
    macro_rules! rows { ($($field:ident),+ $(,)?) => { $(union(&mut left.$field, right.$field);)+ }; }
    rows!(
        areas,
        creatures,
        creature_meta,
        spawns,
        waypoints,
        quests,
        starts,
        ends,
        items,
        objects,
        object_spawns,
        loot,
        vendors,
        equipment,
        scripts,
        item_extras,
        level_stats,
        class_stats
    );
    for pool in right.spawn_pools {
        if let Some(existing) = left.spawn_pools.iter_mut().find(|p| p.id == pool.id) {
            if existing.max_active != pool.max_active {
                bail!("conflicting pool {}", pool.id);
            }
            union(&mut existing.members, pool.members);
            existing.members.sort_by_key(|m| m.guid);
        } else {
            left.spawn_pools.push(pool);
        }
    }
    // A conflicting definition must fail instead of letting runtime hash maps pick one by accident.
    macro_rules! unique {
        ($field:ident, $key:expr) => {{
            let mut seen = std::collections::HashSet::new();
            for row in &left.$field {
                if !seen.insert(($key)(row)) {
                    bail!("conflicting {} definition", stringify!($field));
                }
            }
        }};
    }
    unique!(creatures, |r: &crate::model::Creature| r.entry);
    unique!(creature_meta, |r: &crate::model::CreatureMeta| r.entry);
    unique!(spawns, |r: &crate::model::Spawn| r.guid);
    unique!(quests, |r: &crate::model::Quest| r.id);
    unique!(items, |r: &crate::model::Item| r.entry);
    unique!(objects, |r: &crate::model::ObjectTemplate| r.entry);
    unique!(object_spawns, |r: &crate::model::ObjectSpawn| r.guid);
    left.areas.sort_by_key(|r| r.0);
    left.creatures.sort_by_key(|r| r.entry);
    left.creature_meta.sort_by_key(|r| r.entry);
    left.spawns.sort_by_key(|r| r.guid);
    left.quests.sort_by_key(|r| r.id);
    left.items.sort_by_key(|r| r.entry);
    left.objects.sort_by_key(|r| r.entry);
    left.object_spawns.sort_by_key(|r| r.guid);
    left.spawn_pools.sort_by_key(|r| r.id);
    Ok(left)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PoolMember, SpawnPool};
    #[test]
    fn shared_pool_keeps_both_zones_and_one_active_limit() {
        let make = |guid| World {
            xp_for_level: vec![400, 900, 900],
            spawn_pools: vec![SpawnPool {
                id: 7,
                max_active: 1,
                members: vec![PoolMember { guid, chance: 0.0 }],
            }],
            ..World::default()
        };
        let merged = worlds(make(1), make(2)).unwrap();
        assert_eq!(merged.xp_for_level, vec![400, 900, 900]);
        assert_eq!(merged.spawn_pools[0].max_active, 1);
        assert_eq!(merged.spawn_pools[0].members.len(), 2);
        assert_eq!(worlds(merged.clone(), make(2)).unwrap(), merged);
    }
}
