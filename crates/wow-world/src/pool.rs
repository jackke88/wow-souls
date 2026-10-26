//! Optional creature pool exports, expanded from creature entries to spawn GUIDs.
//! GUID-specific rows take precedence over entry rows in the same pool. A GUID cannot
//! belong to two pools: that would make the exported active limits ambiguous.

use crate::model::{PoolMember, Spawn, SpawnPool};
use crate::tsv::Table;
use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

fn optional_table(dir: &Path, name: &str) -> Result<Option<Table>> {
    let path = dir.join(format!("{name}.tsv"));
    match std::fs::read_to_string(&path) {
        Ok(text) => Table::parse(&text)
            .with_context(|| format!("reading {}", path.display()))
            .map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// Old TSV directories without pool tables remain usable. A present malformed table is an error.
pub fn load_spawn_pools(dir: &Path, spawns: &[Spawn]) -> Result<Vec<SpawnPool>> {
    let templates = optional_table(dir, "pool_template")?;
    let by_guid = optional_table(dir, "pool_creature")?;
    let by_entry = optional_table(dir, "pool_creature_template")?;
    read_spawn_pools(
        templates.as_ref(),
        by_guid.as_ref(),
        by_entry.as_ref(),
        spawns,
    )
}

fn valid_chance(chance: f32) -> Result<()> {
    if !chance.is_finite() || chance < 0.0 {
        bail!("pool chance {chance} must be finite and nonnegative");
    }
    Ok(())
}

/// Check limits, membership uniqueness and chance values in imported or parsed pool records.
pub fn validate(pools: &[SpawnPool]) -> Result<()> {
    let mut ids = HashSet::new();
    let mut owners = HashMap::new();
    for pool in pools {
        if pool.id == 0 || pool.max_active == 0 {
            bail!(
                "pool {} needs a nonzero id and positive max_active",
                pool.id
            );
        }
        if !ids.insert(pool.id) {
            bail!("duplicate pool {}", pool.id);
        }
        for member in &pool.members {
            valid_chance(member.chance)?;
            if member.guid == 0 {
                bail!("pool {} contains GUID zero", pool.id);
            }
            if let Some(previous) = owners.insert(member.guid, pool.id) {
                bail!(
                    "duplicate GUID {} membership in pools {previous} and {}",
                    member.guid,
                    pool.id
                );
            }
        }
    }
    Ok(())
}

/// Read `pool_template(entry,max_limit)`, `pool_creature(guid,pool_entry,chance)` and
/// `pool_creature_template(id,pool_entry,chance)`. Entry pools include primary and alternative
/// creature entries. Rows with no matching exported spawn are irrelevant to this map.
/// The export has already selected the patch; this parser does not filter it again.
pub fn read_spawn_pools(
    templates: Option<&Table>,
    by_guid: Option<&Table>,
    by_entry: Option<&Table>,
    spawns: &[Spawn],
) -> Result<Vec<SpawnPool>> {
    let mut limits = BTreeMap::new();
    if let Some(table) = templates {
        for (n, row) in table.rows().enumerate() {
            let read = || -> Result<(u32, u32)> {
                let id = row.u32("entry")?;
                let limit = row.u32("max_limit")?;
                if id == 0 || limit == 0 {
                    bail!("pool {id} needs a nonzero entry and positive max_limit");
                }
                Ok((id, limit))
            };
            let (id, limit) = read().with_context(|| format!("pool_template row {}", n + 1))?;
            if limits.insert(id, limit).is_some() {
                bail!("pool_template has duplicate entry {id}");
            }
        }
    }
    let guids: HashSet<_> = spawns.iter().map(|spawn| spawn.guid).collect();
    let mut entries: HashMap<u32, HashSet<u32>> = HashMap::new();
    for spawn in spawns {
        for entry in std::iter::once(spawn.entry).chain(spawn.alt) {
            if entry != 0 {
                entries.entry(entry).or_default().insert(spawn.guid);
            }
        }
    }
    let mut members: BTreeMap<u32, BTreeMap<u32, f32>> = BTreeMap::new();
    let mut owners = HashMap::new();
    let mut direct = HashSet::new();
    for (name, table, column, is_direct) in [
        ("pool_creature", by_guid, "guid", true),
        ("pool_creature_template", by_entry, "id", false),
    ] {
        let Some(table) = table else { continue };
        for (n, row) in table.rows().enumerate() {
            let read = || -> Result<(u32, u32, f32)> {
                let key = row.u32(column)?;
                let pool = row.u32("pool_entry")?;
                let chance = row.f32("chance")?;
                valid_chance(chance)?;
                Ok((key, pool, chance))
            };
            let (key, pool, chance) = read().with_context(|| format!("{name} row {}", n + 1))?;
            let matching: Vec<_> = if is_direct {
                guids.contains(&key).then_some(key).into_iter().collect()
            } else {
                entries
                    .get(&key)
                    .map(|guids| guids.iter().copied().collect())
                    .unwrap_or_default()
            };
            if matching.is_empty() {
                continue;
            }
            if !limits.contains_key(&pool) {
                bail!(
                    "{name} row {} references missing pool_template {pool}",
                    n + 1
                );
            }
            for guid in matching {
                if let Some(previous) = owners.insert(guid, pool) {
                    if previous != pool {
                        bail!("GUID {guid} belongs to pools {previous} and {pool}");
                    }
                }
                let pool_members = members.entry(pool).or_default();
                if !is_direct && direct.contains(&guid) {
                    continue;
                }
                if let Some(previous) = pool_members.insert(guid, chance) {
                    if previous != chance {
                        bail!("GUID {guid} has conflicting chances in pool {pool}");
                    }
                }
                if is_direct {
                    direct.insert(guid);
                }
            }
        }
    }
    let pools: Vec<_> = members
        .into_iter()
        .map(|(id, members)| SpawnPool {
            id,
            max_active: limits[&id],
            members: members
                .into_iter()
                .map(|(guid, chance)| PoolMember { guid, chance })
                .collect(),
        })
        .collect();
    validate(&pools)?;
    Ok(pools)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawns() -> Vec<Spawn> {
        [
            (31, 448, [0; 4]),
            (11, 448, [61, 0, 0, 0]),
            (21, 61, [448, 448, 0, 0]),
        ]
        .into_iter()
        .map(|(guid, entry, alt)| Spawn {
            guid,
            entry,
            alt,
            pos: [0.0; 3],
            orientation: 0.0,
            respawn_min: 300,
            respawn_max: 300,
            wander: 0.0,
            movement: 0,
            area: 12,
            event: false,
        })
        .collect()
    }

    #[test]
    fn entry_pools_expand_alternatives_once_and_preserve_the_active_limit() {
        let templates = Table::parse("entry\tmax_limit\n1270\t1\n").unwrap();
        let entries = Table::parse("id\tpool_entry\tchance\n448\t1270\t0\n").unwrap();
        let pools = read_spawn_pools(Some(&templates), None, Some(&entries), &spawns()).unwrap();
        assert_eq!(pools.len(), 1);
        assert_eq!((pools[0].id, pools[0].max_active), (1270, 1));
        assert_eq!(
            pools[0].members.iter().map(|m| m.guid).collect::<Vec<_>>(),
            vec![11, 21, 31]
        );
        assert!(pools[0].members.iter().all(|m| m.chance == 0.0));
    }

    #[test]
    fn direct_members_override_entry_chance_without_duplicate_guids() {
        let templates = Table::parse("entry\tmax_limit\n1270\t1\n").unwrap();
        let direct = Table::parse("guid\tpool_entry\tchance\n11\t1270\t25\n").unwrap();
        let entries = Table::parse("id\tpool_entry\tchance\n448\t1270\t0\n").unwrap();
        let pools =
            read_spawn_pools(Some(&templates), Some(&direct), Some(&entries), &spawns()).unwrap();
        assert_eq!(pools[0].members.len(), 3);
        assert_eq!(
            pools[0].members[0],
            PoolMember {
                guid: 11,
                chance: 25.0
            }
        );
    }

    #[test]
    fn missing_pool_tables_and_unrelated_map_members_are_ignored() {
        assert!(read_spawn_pools(None, None, None, &spawns())
            .unwrap()
            .is_empty());
        let direct = Table::parse("guid\tpool_entry\tchance\n999\t90\t0\n").unwrap();
        let entries = Table::parse("id\tpool_entry\tchance\n999\t90\t0\n").unwrap();
        assert!(
            read_spawn_pools(None, Some(&direct), Some(&entries), &spawns())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn missing_relevant_template_is_an_error_for_each_member_source() {
        for (direct, text) in [
            (true, "guid\tpool_entry\tchance\n11\t1270\t0\n"),
            (false, "id\tpool_entry\tchance\n448\t1270\t0\n"),
        ] {
            let table = Table::parse(text).unwrap();
            let err = read_spawn_pools(
                None,
                direct.then_some(&table),
                (!direct).then_some(&table),
                &spawns(),
            )
            .unwrap_err()
            .to_string();
            assert!(err.contains("missing pool_template 1270"), "{err}");
        }
    }

    #[test]
    fn bad_limits_and_chances_are_rejected() {
        for limit in ["0", "-1", "word", "4294967296"] {
            let table = Table::parse(&format!("entry\tmax_limit\n1270\t{limit}\n")).unwrap();
            assert!(read_spawn_pools(Some(&table), None, None, &spawns()).is_err());
        }
        let templates = Table::parse("entry\tmax_limit\n1270\t1\n").unwrap();
        for chance in ["-1", "NaN", "inf", "-inf"] {
            let table =
                Table::parse(&format!("guid\tpool_entry\tchance\n11\t1270\t{chance}\n")).unwrap();
            assert!(read_spawn_pools(Some(&templates), Some(&table), None, &spawns()).is_err());
        }
    }

    #[test]
    fn a_guid_cannot_be_expanded_into_two_pools() {
        let templates = Table::parse("entry\tmax_limit\n1270\t1\n3200\t1\n").unwrap();
        let entries = Table::parse("id\tpool_entry\tchance\n448\t1270\t0\n61\t3200\t0\n").unwrap();
        let err = read_spawn_pools(Some(&templates), None, Some(&entries), &spawns())
            .unwrap_err()
            .to_string();
        assert!(err.contains("belongs to pools"), "{err}");
    }

    #[test]
    fn filesystem_loader_accepts_missing_exports_and_reports_present_bad_tables() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("wow-world-pools-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        assert!(load_spawn_pools(&dir, &spawns()).unwrap().is_empty());
        let path = dir.join("pool_creature_template.tsv");
        std::fs::write(&path, "id\tpool_entry\tchance\n448\t1270\t0\n").unwrap();
        assert!(load_spawn_pools(&dir, &spawns()).is_err());
        let template = dir.join("pool_template.tsv");
        std::fs::write(&template, "entry\tmax_limit\n1270\t1\n").unwrap();
        assert_eq!(
            load_spawn_pools(&dir, &spawns()).unwrap()[0].members.len(),
            3
        );
        std::fs::write(&path, "id\tpool_entry\tchance\n448\t1270\n").unwrap();
        let err = load_spawn_pools(&dir, &spawns()).unwrap_err().to_string();
        assert!(err.contains("pool_creature_template.tsv"), "{err}");
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(template).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
