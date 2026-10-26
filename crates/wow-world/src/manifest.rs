//! The manifest `wsworld 4`: the selected [`World`] as text, the one file the game reads.
//!
//! The first line is `wsworld 4` (the reader also takes versions 1–3). Every other line is
//! one record, `kind<TAB>field<TAB>...`, escaped like the database export (`tsv::escape`: a backslash
//! before a backslash, tab or newline; an empty string is an empty field). The fields of a kind are
//! the fields of its `model.rs` struct in declaration order: arrays are flattened into consecutive
//! fields, a list (`Vec`) is its length followed by its entries (a pair is two fields), a `bool` is
//! `0` or `1`, numbers use Rust's `{}` formatting (which reads back to the same value, bit for bit).
//! Kinds: `area`, `creature`, `spawn`, `waypoint`, `quest`, `start`, `end`, `item`, `itemx`, `object`,
//! `objspawn`, `loot`, `vendor`, `equip`, `qscript`, `xp`. A `loot` row begins with its table family:
//! `creature`, `object` or `reference`. An `xp` record is `xp<TAB>level<TAB>xp to the next level`, the
//! levels counting up from 1.
//!
//! **Versions.** `wsworld 2` only appends to `wsworld 1`: the `quest` record gains `max_level`,
//! `required_condition`, `rep_objective`, `rep_min`, `rep_max`, `reward_rep` and `reward_mail` after
//! `event`; the `object` record gains `data6..data9` after `data5`; and the kinds `itemx`, `qscript` and
//! `xp` are new. The reader takes a `wsworld 1` file too (the missing fields read as 0 / empty and the new
//! kinds are refused in it). Version 3 adds item stats and the `levelstat` / `classstat` kinds.
//! Version 4 adds separate `creaturemeta` records (entry, four display weights, reaction
//! `unknown` / `friendly` / `neutral` / `hostile`) and `pool` records (id, active limit, member count,
//! then GUID / chance pairs). Their vectors stay empty in older files. The writer writes version 4.
//!
//! The text is a function of the `World` (vectors are written in their order, `select` sorts them),
//! so writing the same world twice gives the same bytes.

use crate::model::*;
use crate::tsv::{escape, records};
use anyhow::{anyhow, bail, Result};
use std::fmt::Display;
use std::str::FromStr;

const MAGIC: &str = "wsworld";
/// The version the writer writes.
const VERSION: &str = "4";
/// The versions the reader knows.
const READS: [&str; 4] = ["1", "2", "3", "4"];

/// One record being written.
struct Rec(Vec<String>);

impl Rec {
    fn new(kind: &str) -> Rec {
        Rec(vec![kind.to_string()])
    }

    fn put(&mut self, v: impl Display) -> &mut Rec {
        self.0.push(v.to_string());
        self
    }

    fn flag(&mut self, v: bool) -> &mut Rec {
        self.put(u8::from(v))
    }

    fn all<T: Display>(&mut self, v: &[T]) -> &mut Rec {
        for x in v {
            self.put(x);
        }
        self
    }

    /// A list: its length, then the entries.
    fn list<T: Display>(&mut self, v: &[T]) -> &mut Rec {
        self.put(v.len());
        self.all(v)
    }

    /// A list of pairs: its length, then both halves of every entry.
    fn pairs<A: Display, B: Display>(&mut self, v: &[(A, B)]) -> &mut Rec {
        self.put(v.len());
        for (a, b) in v {
            self.put(a).put(b);
        }
        self
    }

    fn line(&self) -> String {
        let mut s = self
            .0
            .iter()
            .map(|f| escape(f))
            .collect::<Vec<_>>()
            .join("\t");
        s.push('\n');
        s
    }
}

fn loot_kind_name(k: LootKind) -> &'static str {
    match k {
        LootKind::Creature => "creature",
        LootKind::Object => "object",
        LootKind::Reference => "reference",
    }
}

/// The whole world as `wsworld 4` text.
pub fn write(w: &World) -> String {
    let mut out = format!("{MAGIC} {VERSION}\n");
    let mut add = |r: &Rec| out.push_str(&r.line());
    for (id, name) in &w.areas {
        add(Rec::new("area").put(id).put(name));
    }
    for c in &w.creatures {
        add(Rec::new("creature")
            .put(c.entry)
            .put(&c.name)
            .put(&c.subname)
            .put(c.level_min)
            .put(c.level_max)
            .put(c.faction)
            .put(c.npc_flags)
            .all(&c.display)
            .all(&c.scale)
            .put(c.kind)
            .put(c.rank)
            .put(c.detection_range)
            .put(c.call_for_help_range)
            .put(c.leash_range)
            .put(c.speed_walk)
            .put(c.speed_run)
            .put(c.loot_id)
            .put(c.equipment_id)
            .put(c.vendor_id)
            .put(c.trainer_type)
            .put(c.trainer_class)
            .put(c.movement_type)
            .flag(c.civilian)
            .put(c.health_multiplier)
            .put(c.damage_multiplier)
            .put(c.unit_class));
    }
    for meta in &w.creature_meta {
        let reaction = match meta.reaction {
            None => "unknown",
            Some(Disposition::Friendly) => "friendly",
            Some(Disposition::Neutral) => "neutral",
            Some(Disposition::Hostile) => "hostile",
        };
        add(Rec::new("creaturemeta")
            .put(meta.entry)
            .all(&meta.display_weights)
            .put(reaction));
    }
    for pool in &w.spawn_pools {
        let mut rec = Rec::new("pool");
        rec.put(pool.id)
            .put(pool.max_active)
            .put(pool.members.len());
        for member in &pool.members {
            rec.put(member.guid).put(member.chance);
        }
        add(&rec);
    }
    for s in &w.spawns {
        add(Rec::new("spawn")
            .put(s.guid)
            .put(s.entry)
            .all(&s.alt)
            .all(&s.pos)
            .put(s.orientation)
            .put(s.respawn_min)
            .put(s.respawn_max)
            .put(s.wander)
            .put(s.movement)
            .put(s.area)
            .flag(s.event));
    }
    for p in &w.waypoints {
        add(Rec::new("waypoint")
            .put(p.guid)
            .put(p.point)
            .all(&p.pos)
            .put(p.orientation)
            .put(p.wait_ms)
            .put(p.script_id)
            .put(p.path_id));
    }
    for q in &w.quests {
        add(Rec::new("quest")
            .put(q.id)
            .put(q.patch)
            .put(&q.title)
            .put(&q.details)
            .put(&q.objectives)
            .put(&q.offer_reward)
            .put(&q.request_items)
            .put(&q.end_text)
            .all(&q.objective_text)
            .put(q.zone_or_sort)
            .put(q.min_level)
            .put(q.level)
            .put(q.required_classes)
            .put(q.required_races)
            .put(q.prev)
            .put(q.next)
            .put(q.exclusive_group)
            .put(q.next_in_chain)
            .put(q.flags)
            .put(q.special_flags)
            .put(q.limit_time)
            .put(q.src_item)
            .put(q.src_item_count)
            .pairs(&q.req_items)
            .pairs(&q.req_kills)
            .list(&q.req_spell_cast)
            .pairs(&q.reward_choice)
            .pairs(&q.reward_items)
            .put(q.reward_money)
            .put(q.reward_xp)
            .put(q.reward_spell)
            .put(q.point.0)
            .put(q.point.1)
            .put(q.point.2)
            .put(q.start_script)
            .put(q.complete_script)
            .flag(q.event)
            .put(q.max_level)
            .put(q.required_condition)
            .put(q.rep_objective.0)
            .put(q.rep_objective.1)
            .put(q.rep_min.0)
            .put(q.rep_min.1)
            .put(q.rep_max.0)
            .put(q.rep_max.1)
            .pairs(&q.reward_rep)
            .put(q.reward_mail.0)
            .put(q.reward_mail.1)
            .put(q.reward_mail.2));
    }
    for (kind, rows) in [("start", &w.starts), ("end", &w.ends)] {
        for r in rows {
            add(Rec::new(kind).put(r.npc).put(r.quest).flag(r.object));
        }
    }
    for s in &w.scripts {
        add(Rec::new("qscript")
            .put(s.quest)
            .flag(s.start)
            .put(s.delay)
            .put(s.priority)
            .put(s.command)
            .all(&s.datalong)
            .all(&s.target_param)
            .put(s.target_type)
            .put(s.data_flags)
            .all(&s.dataint)
            .all(&s.pos)
            .put(s.condition_id)
            .put(&s.comments));
    }
    for i in &w.items {
        add(Rec::new("item")
            .put(i.entry)
            .put(&i.name)
            .put(i.class)
            .put(i.subclass)
            .put(i.display)
            .put(i.quality)
            .put(i.flags)
            .put(i.buy_price)
            .put(i.sell_price)
            .put(i.inventory_type)
            .put(i.allowable_class)
            .put(i.allowable_race)
            .put(i.item_level)
            .put(i.required_level)
            .put(i.max_count)
            .put(i.stackable)
            .put(i.delay)
            .put(i.damage.0)
            .put(i.damage.1)
            .put(i.damage.2)
            .put(i.block)
            .put(i.armor)
            .pairs(&i.stats));
    }
    for i in &w.item_extras {
        add(Rec::new("itemx")
            .put(i.entry)
            .put(i.start_quest)
            .put(i.bonding));
    }
    for o in &w.objects {
        add(Rec::new("object")
            .put(o.entry)
            .put(&o.name)
            .put(o.kind)
            .put(o.display)
            .put(o.faction)
            .put(o.flags)
            .put(o.size)
            .all(&o.data));
    }
    for s in &w.object_spawns {
        add(Rec::new("objspawn")
            .put(s.guid)
            .put(s.entry)
            .all(&s.pos)
            .put(s.orientation)
            .put(s.respawn_min)
            .put(s.respawn_max)
            .put(s.area)
            .flag(s.event));
    }
    for l in &w.loot {
        add(Rec::new("loot")
            .put(loot_kind_name(l.kind))
            .put(l.table)
            .put(l.item)
            .put(l.chance)
            .put(l.group)
            .put(l.min_or_ref)
            .put(l.max_count));
    }
    for v in &w.vendors {
        add(Rec::new("vendor")
            .put(v.npc)
            .put(v.item)
            .put(v.max_count)
            .put(v.incr_time));
    }
    for e in &w.equipment {
        add(Rec::new("equip")
            .put(e.entry)
            .put(e.probability)
            .all(&e.items));
    }
    for (i, need) in w.xp_for_level.iter().enumerate() {
        add(Rec::new("xp").put(i + 1).put(need));
    }
    for s in &w.level_stats {
        add(Rec::new("levelstat")
            .put(s.class)
            .put(s.level)
            .put(s.strength)
            .put(s.agility)
            .put(s.stamina)
            .put(s.intellect)
            .put(s.spirit));
    }
    for s in &w.class_stats {
        add(Rec::new("classstat")
            .put(s.class)
            .put(s.level)
            .put(s.base_hp)
            .put(s.base_mana));
    }
    out
}

/// The fields of one record being read, front to back.
struct Cur<'a> {
    fields: &'a [Option<String>],
    at: usize,
    /// The file's `wsworld` version (1–4).
    version: u8,
}

impl<'a> Cur<'a> {
    /// A field group that `wsworld` version `since` appended to its record: read it from a file of
    /// that version or later, the type's default (0, empty) from an older one.
    fn since<T: Default>(
        &mut self,
        since: u8,
        read: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        if self.version >= since {
            read(self)
        } else {
            Ok(T::default())
        }
    }

    fn raw(&mut self) -> Result<&'a str> {
        let Some(f) = self.fields.get(self.at) else {
            bail!("too few fields (field {} is missing)", self.at + 1);
        };
        self.at += 1;
        Ok(f.as_deref().unwrap_or(""))
    }

    fn text(&mut self) -> Result<String> {
        Ok(self.raw()?.to_string())
    }

    fn num<T: FromStr>(&mut self) -> Result<T>
    where
        T::Err: Display,
    {
        let n = self.at + 1;
        let t = self.raw()?;
        t.parse().map_err(|e| anyhow!("field {n} = `{t}`: {e}"))
    }

    fn flag(&mut self) -> Result<bool> {
        let n = self.at + 1;
        match self.raw()? {
            "0" => Ok(false),
            "1" => Ok(true),
            t => bail!("field {n} = `{t}`: a flag is 0 or 1"),
        }
    }

    fn arr<T: FromStr, const N: usize>(&mut self) -> Result<[T; N]>
    where
        T::Err: Display,
    {
        let mut v = Vec::with_capacity(N);
        for _ in 0..N {
            v.push(self.num::<T>()?);
        }
        v.try_into()
            .map_err(|_| anyhow!("an array of {N} was not filled"))
    }

    fn list<T: FromStr>(&mut self) -> Result<Vec<T>>
    where
        T::Err: Display,
    {
        let n: usize = self.num()?;
        (0..n).map(|_| self.num()).collect()
    }

    fn pairs<A: FromStr, B: FromStr>(&mut self) -> Result<Vec<(A, B)>>
    where
        A::Err: Display,
        B::Err: Display,
    {
        let n: usize = self.num()?;
        (0..n).map(|_| Ok((self.num()?, self.num()?))).collect()
    }

    fn done(&self) -> Result<()> {
        if self.at != self.fields.len() {
            bail!(
                "{} extra field(s) after field {}",
                self.fields.len() - self.at,
                self.at
            );
        }
        Ok(())
    }
}

fn loot_kind(name: &str) -> Result<LootKind> {
    Ok(match name {
        "creature" => LootKind::Creature,
        "object" => LootKind::Object,
        "reference" => LootKind::Reference,
        other => bail!("unknown loot family `{other}` (creature, object or reference)"),
    })
}

/// Read one record of `kind` into `w`. Fields are read in the order of the struct literal, which is
/// the order `write` puts them out in.
fn read_record(kind: &str, c: &mut Cur, w: &mut World) -> Result<()> {
    if matches!(kind, "itemx" | "qscript" | "xp") && c.version < 2 {
        bail!("unknown record kind `{kind}` (it needs wsworld 2)");
    }
    if matches!(kind, "levelstat" | "classstat") && c.version < 3 {
        bail!("unknown record kind `{kind}` (it needs wsworld 3)");
    }
    if matches!(kind, "creaturemeta" | "pool") && c.version < 4 {
        bail!("unknown record kind `{kind}` (it needs wsworld 4)");
    }
    match kind {
        "area" => w.areas.push((c.num()?, c.text()?)),
        "creaturemeta" => w.creature_meta.push(CreatureMeta {
            entry: c.num()?,
            display_weights: c.arr()?,
            reaction: match c.raw()? {
                "unknown" => None,
                "friendly" => Some(Disposition::Friendly),
                "neutral" => Some(Disposition::Neutral),
                "hostile" => Some(Disposition::Hostile),
                other => bail!("unknown creature reaction `{other}`"),
            },
        }),
        "pool" => {
            let id = c.num()?;
            let max_active = c.num()?;
            let n: usize = c.num()?;
            if n > (c.fields.len() - c.at) / 2 {
                bail!("too few fields for {n} pool members");
            }
            let members = (0..n)
                .map(|_| {
                    Ok(PoolMember {
                        guid: c.num()?,
                        chance: c.num()?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            w.spawn_pools.push(SpawnPool {
                id,
                max_active,
                members,
            });
        }
        "creature" => w.creatures.push(Creature {
            entry: c.num()?,
            name: c.text()?,
            subname: c.text()?,
            level_min: c.num()?,
            level_max: c.num()?,
            faction: c.num()?,
            npc_flags: c.num()?,
            display: c.arr()?,
            scale: c.arr()?,
            kind: c.num()?,
            rank: c.num()?,
            detection_range: c.num()?,
            call_for_help_range: c.num()?,
            leash_range: c.num()?,
            speed_walk: c.num()?,
            speed_run: c.num()?,
            loot_id: c.num()?,
            equipment_id: c.num()?,
            vendor_id: c.num()?,
            trainer_type: c.num()?,
            trainer_class: c.num()?,
            movement_type: c.num()?,
            civilian: c.flag()?,
            health_multiplier: c.num()?,
            damage_multiplier: c.num()?,
            unit_class: c.num()?,
        }),
        "spawn" => w.spawns.push(Spawn {
            guid: c.num()?,
            entry: c.num()?,
            alt: c.arr()?,
            pos: c.arr()?,
            orientation: c.num()?,
            respawn_min: c.num()?,
            respawn_max: c.num()?,
            wander: c.num()?,
            movement: c.num()?,
            area: c.num()?,
            event: c.flag()?,
        }),
        "waypoint" => w.waypoints.push(Waypoint {
            guid: c.num()?,
            point: c.num()?,
            pos: c.arr()?,
            orientation: c.num()?,
            wait_ms: c.num()?,
            script_id: c.num()?,
            path_id: c.num()?,
        }),
        "quest" => w.quests.push(Quest {
            id: c.num()?,
            patch: c.num()?,
            title: c.text()?,
            details: c.text()?,
            objectives: c.text()?,
            offer_reward: c.text()?,
            request_items: c.text()?,
            end_text: c.text()?,
            objective_text: c.arr()?,
            zone_or_sort: c.num()?,
            min_level: c.num()?,
            level: c.num()?,
            required_classes: c.num()?,
            required_races: c.num()?,
            prev: c.num()?,
            next: c.num()?,
            exclusive_group: c.num()?,
            next_in_chain: c.num()?,
            flags: c.num()?,
            special_flags: c.num()?,
            limit_time: c.num()?,
            src_item: c.num()?,
            src_item_count: c.num()?,
            req_items: c.pairs()?,
            req_kills: c.pairs()?,
            req_spell_cast: c.list()?,
            reward_choice: c.pairs()?,
            reward_items: c.pairs()?,
            reward_money: c.num()?,
            reward_xp: c.num()?,
            reward_spell: c.num()?,
            point: (c.num()?, c.num()?, c.num()?),
            start_script: c.num()?,
            complete_script: c.num()?,
            event: c.flag()?,
            max_level: c.since(2, |c| c.num())?,
            required_condition: c.since(2, |c| c.num())?,
            rep_objective: c.since(2, |c| Ok((c.num()?, c.num()?)))?,
            rep_min: c.since(2, |c| Ok((c.num()?, c.num()?)))?,
            rep_max: c.since(2, |c| Ok((c.num()?, c.num()?)))?,
            reward_rep: c.since(2, |c| c.pairs())?,
            reward_mail: c.since(2, |c| Ok((c.num()?, c.num()?, c.num()?)))?,
        }),
        "qscript" => w.scripts.push(QuestScript {
            quest: c.num()?,
            start: c.flag()?,
            delay: c.num()?,
            priority: c.num()?,
            command: c.num()?,
            datalong: c.arr()?,
            target_param: c.arr()?,
            target_type: c.num()?,
            data_flags: c.num()?,
            dataint: c.arr()?,
            pos: c.arr()?,
            condition_id: c.num()?,
            comments: c.text()?,
        }),
        "xp" => {
            let level: usize = c.num()?;
            if level != w.xp_for_level.len() + 1 {
                bail!(
                    "xp level {level} after {} row(s): the levels count up from 1",
                    w.xp_for_level.len()
                );
            }
            w.xp_for_level.push(c.num()?);
        }
        "start" | "end" => {
            let r = Relation {
                npc: c.num()?,
                quest: c.num()?,
                object: c.flag()?,
            };
            if kind == "start" {
                w.starts.push(r);
            } else {
                w.ends.push(r);
            }
        }
        "item" => w.items.push(Item {
            entry: c.num()?,
            name: c.text()?,
            class: c.num()?,
            subclass: c.num()?,
            display: c.num()?,
            quality: c.num()?,
            flags: c.num()?,
            buy_price: c.num()?,
            sell_price: c.num()?,
            inventory_type: c.num()?,
            allowable_class: c.num()?,
            allowable_race: c.num()?,
            item_level: c.num()?,
            required_level: c.num()?,
            max_count: c.num()?,
            stackable: c.num()?,
            delay: c.num()?,
            damage: (c.num()?, c.num()?, c.num()?),
            block: c.num()?,
            armor: c.num()?,
            stats: c.since(3, |c| c.pairs())?,
        }),
        "levelstat" => w.level_stats.push(LevelStat {
            class: c.num()?,
            level: c.num()?,
            strength: c.num()?,
            agility: c.num()?,
            stamina: c.num()?,
            intellect: c.num()?,
            spirit: c.num()?,
        }),
        "classstat" => w.class_stats.push(ClassStat {
            class: c.num()?,
            level: c.num()?,
            base_hp: c.num()?,
            base_mana: c.num()?,
        }),
        "itemx" => w.item_extras.push(ItemExtra {
            entry: c.num()?,
            start_quest: c.num()?,
            bonding: c.num()?,
        }),
        "object" => w.objects.push(ObjectTemplate {
            entry: c.num()?,
            name: c.text()?,
            kind: c.num()?,
            display: c.num()?,
            faction: c.num()?,
            flags: c.num()?,
            size: c.num()?,
            data: {
                let head: [i32; 6] = c.arr()?;
                let tail: [i32; 4] = c.since(2, |c| c.arr())?;
                let mut data = [0; 10];
                data[..6].copy_from_slice(&head);
                data[6..].copy_from_slice(&tail);
                data
            },
        }),
        "objspawn" => w.object_spawns.push(ObjectSpawn {
            guid: c.num()?,
            entry: c.num()?,
            pos: c.arr()?,
            orientation: c.num()?,
            respawn_min: c.num()?,
            respawn_max: c.num()?,
            area: c.num()?,
            event: c.flag()?,
        }),
        "loot" => w.loot.push(LootRow {
            kind: loot_kind(c.raw()?)?,
            table: c.num()?,
            item: c.num()?,
            chance: c.num()?,
            group: c.num()?,
            min_or_ref: c.num()?,
            max_count: c.num()?,
        }),
        "vendor" => w.vendors.push(Vendor {
            npc: c.num()?,
            item: c.num()?,
            max_count: c.num()?,
            incr_time: c.num()?,
        }),
        "equip" => w.equipment.push(Equipment {
            entry: c.num()?,
            probability: c.num()?,
            items: c.arr()?,
        }),
        other => bail!("unknown record kind `{other}`"),
    }
    c.done()
}

/// Read any supported `wsworld` version back into a [`World`].
pub fn parse(text: &str) -> Result<World> {
    let recs = records(text)?;
    let Some(first) = recs.first() else {
        bail!("manifest: empty, no `{MAGIC} {VERSION}` line");
    };
    let head = match first.as_slice() {
        [Some(line)] => line.as_str(),
        _ => bail!("manifest: the first line is not `{MAGIC} <version>`"),
    };
    let Some((magic, version)) = head.split_once(' ') else {
        bail!("manifest: the first line `{head}` is not `{MAGIC} <version>`");
    };
    if magic != MAGIC {
        bail!("manifest: the first line `{head}` is not `{MAGIC} <version>`");
    }
    let Some(version) = READS
        .iter()
        .position(|&v| v == version)
        .map(|i| i as u8 + 1)
    else {
        bail!(
            "manifest: unsupported version `{version}` (this reader knows `{MAGIC} {}`)",
            READS.join("` and `")
        );
    };
    let mut w = World::default();
    for (n, rec) in recs.iter().enumerate().skip(1) {
        let kind = rec[0].as_deref().unwrap_or("");
        let mut cur = Cur {
            fields: &rec[1..],
            at: 0,
            version,
        };
        read_record(kind, &mut cur, &mut w)
            .map_err(|e| anyhow!("manifest record {n} (`{kind}`): {e}"))?;
    }
    crate::pool::validate(&w.spawn_pools).map_err(|e| anyhow!("manifest pools: {e}"))?;
    let mut entries = std::collections::HashSet::new();
    for meta in &w.creature_meta {
        if !entries.insert(meta.entry) {
            bail!("manifest: duplicate creaturemeta entry {}", meta.entry);
        }
    }
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> World {
        World {
            areas: vec![(9, "Northshire Valley".into())],
            spawns: vec![Spawn {
                guid: 80247,
                entry: 103,
                alt: [0; 4],
                pos: [-9056.48, -460.903, 72.8318],
                orientation: 1.5,
                respawn_min: 270,
                respawn_max: 270,
                wander: 0.0,
                movement: 0,
                area: 9,
                event: false,
            }],
            quests: vec![Quest {
                id: 18,
                patch: 0,
                title: "Brotherhood of Thieves".into(),
                details: "Hello $N,\nit\ttabs\\slash".into(),
                objectives: String::new(),
                offer_reward: "Thanks".into(),
                request_items: "Not yet".into(),
                end_text: String::new(),
                objective_text: Default::default(),
                zone_or_sort: 9,
                min_level: 2,
                level: 4,
                required_classes: 0,
                required_races: 0,
                prev: 783,
                next: 3903,
                exclusive_group: -18,
                next_in_chain: 0,
                flags: 0,
                special_flags: 0,
                limit_time: 0,
                src_item: 0,
                src_item_count: 0,
                req_items: vec![(752, 12)],
                req_kills: vec![(-6, 3)],
                req_spell_cast: vec![],
                reward_choice: vec![(2224, 1), (5580, 1)],
                reward_items: vec![],
                reward_money: 25,
                reward_xp: 355,
                reward_spell: 0,
                point: (0, 0.0, 0.0),
                start_script: 0,
                complete_script: 0,
                event: false,
                max_level: 0,
                required_condition: 0,
                rep_objective: (0, 0),
                rep_min: (0, 0),
                rep_max: (0, 0),
                reward_rep: vec![(72, 100)],
                reward_mail: (0, 0, 0),
            }],
            ..World::default()
        }
    }

    /// Every record kind at least once, every field a different value so a swapped pair is caught.
    fn full() -> World {
        let mut w = sample();
        w.areas.push((86, "Northshire\tAbbey".into()));
        w.creatures = vec![
            Creature {
                entry: 38,
                name: "Defias Thug".into(),
                subname: String::new(),
                level_min: 2,
                level_max: 3,
                faction: 12,
                npc_flags: 7,
                display: [5035, 5036, 5037, 5038],
                scale: [1.0, 1.1, 1.2, 1.3],
                kind: 5,
                rank: 1,
                detection_range: 20.5,
                call_for_help_range: 5.25,
                leash_range: 40.0,
                speed_walk: 1.0,
                speed_run: 1.14286,
                loot_id: 38,
                equipment_id: 4,
                vendor_id: 9,
                trainer_type: 2,
                trainer_class: 3,
                movement_type: 1,
                civilian: false,
                health_multiplier: 1.5,
                damage_multiplier: 1.2,
                unit_class: 1,
            },
            Creature {
                entry: 721,
                name: "Rabbit\\Tab\t".into(),
                subname: "Critter".into(),
                level_min: 1,
                level_max: 1,
                faction: 31,
                npc_flags: 0,
                display: [328, 0, 0, 0],
                scale: [0.7, 0.0, 0.0, 0.0],
                kind: 8,
                rank: 0,
                detection_range: 10.0,
                call_for_help_range: 0.0,
                leash_range: 0.0,
                speed_walk: 1.0,
                speed_run: 1.0,
                loot_id: 0,
                equipment_id: 0,
                vendor_id: 0,
                trainer_type: 0,
                trainer_class: 0,
                movement_type: 1,
                civilian: true,
                health_multiplier: 1.0,
                damage_multiplier: 1.0,
                unit_class: 1,
            },
        ];
        w.creature_meta = vec![
            CreatureMeta {
                entry: 38,
                display_weights: [10, 20, 30, 40],
                reaction: Some(Disposition::Hostile),
            },
            CreatureMeta {
                entry: 721,
                display_weights: [100, 0, 0, 0],
                reaction: Some(Disposition::Neutral),
            },
            CreatureMeta {
                entry: 197,
                display_weights: [0, 50, 0, 50],
                reaction: Some(Disposition::Friendly),
            },
            CreatureMeta {
                entry: 103,
                display_weights: [0; 4],
                reaction: None,
            },
        ];
        w.spawn_pools = vec![SpawnPool {
            id: 1270,
            max_active: 1,
            members: vec![
                PoolMember {
                    guid: 80247,
                    chance: 0.0,
                },
                PoolMember {
                    guid: 6732,
                    chance: 0.1,
                },
            ],
        }];
        w.spawns.push(Spawn {
            guid: 6732,
            entry: 721,
            alt: [11, 12, 13, 14],
            // Awkward floats must come back bit for bit.
            pos: [0.1, 1.0e-7, f32::MAX],
            orientation: -3.11049,
            respawn_min: 120,
            respawn_max: 180,
            wander: 5.5,
            movement: 2,
            area: 86,
            event: true,
        });
        w.waypoints = vec![
            Waypoint {
                guid: 6732,
                point: 1,
                pos: [-9638.02, -3164.06, 49.1948],
                orientation: 3.11049,
                wait_ms: 5000,
                script_id: 7,
                path_id: 2,
            },
            Waypoint {
                guid: 6732,
                point: 2,
                pos: [0.1, -1.0e-7, f32::MAX],
                orientation: 0.0,
                wait_ms: 0,
                script_id: 0,
                path_id: 2,
            },
        ];
        w.quests.push(Quest {
            id: 7,
            patch: 4,
            title: "Kobold Camp Cleanup".into(),
            details: "line one\nline two\n".into(),
            objectives: "Kill 8 \u{201C}Kobold Vermin\u{201D}".into(),
            offer_reward: "Åäö".into(),
            request_items: "r".into(),
            end_text: "e".into(),
            objective_text: ["o1".into(), String::new(), "o3".into(), "o\\4".into()],
            zone_or_sort: -141,
            min_level: 1,
            level: -1,
            required_classes: 1,
            required_races: 2,
            prev: -3,
            next: 4,
            exclusive_group: 5,
            next_in_chain: 6,
            flags: 7,
            special_flags: 8,
            limit_time: 9,
            src_item: 10,
            src_item_count: 11,
            req_items: vec![(12, 13), (14, 15)],
            req_kills: vec![(16, 17), (-18, 19)],
            req_spell_cast: vec![20, 21],
            reward_choice: vec![(22, 23)],
            reward_items: vec![(24, 25), (26, 27)],
            reward_money: -28,
            reward_xp: 29,
            reward_spell: 30,
            point: (31, 32.5, -33.25),
            start_script: 34,
            complete_script: 35,
            event: true,
            max_level: 36,
            required_condition: 37,
            rep_objective: (38, -39),
            rep_min: (40, -41),
            rep_max: (42, 43),
            reward_rep: vec![(44, 45), (46, -47)],
            reward_mail: (48, 49, 50),
        });
        w.scripts = vec![
            QuestScript {
                quest: 7,
                start: true,
                delay: 1,
                priority: 2,
                command: 15,
                datalong: [6245, 3, 4, 5],
                target_param: [6, 7],
                target_type: 8,
                data_flags: 9,
                dataint: [-10, 11, 0, -12],
                pos: [13.5, -14.25, 1.0e-7, f32::MAX],
                condition_id: 15,
                comments: "Report\tto\\Goldshire\nSalute".into(),
            },
            QuestScript {
                quest: 7,
                start: false,
                delay: 0,
                priority: 0,
                command: 0,
                datalong: [0; 4],
                target_param: [0; 2],
                target_type: 0,
                data_flags: 0,
                dataint: [1242, 0, 0, 0],
                pos: [0.0; 4],
                condition_id: 0,
                comments: String::new(),
            },
        ];
        w.xp_for_level = vec![400, 900, 1400];
        w.level_stats = vec![
            LevelStat {
                class: 1,
                level: 1,
                strength: 23,
                agility: 20,
                stamina: 22,
                intellect: 20,
                spirit: 20,
            },
            LevelStat {
                class: 1,
                level: 2,
                strength: 24,
                agility: 21,
                stamina: 23,
                intellect: 20,
                spirit: 20,
            },
        ];
        w.class_stats = vec![
            ClassStat {
                class: 1,
                level: 1,
                base_hp: 20,
                base_mana: 0,
            },
            ClassStat {
                class: 1,
                level: 2,
                base_hp: 29,
                base_mana: 0,
            },
        ];
        w.item_extras = vec![
            ItemExtra {
                entry: 2092,
                start_quest: 5805,
                bonding: 4,
            },
            ItemExtra {
                entry: 14646,
                start_quest: 0,
                bonding: 1,
            },
        ];
        w.starts = vec![
            Relation {
                npc: 197,
                quest: 7,
                object: false,
            },
            Relation {
                npc: 61,
                quest: 5,
                object: true,
            },
        ];
        w.ends = vec![Relation {
            npc: 823,
            quest: 18,
            object: false,
        }];
        w.items = vec![Item {
            entry: 2092,
            name: "Worn Dagger".into(),
            class: 2,
            subclass: 15,
            display: 6544,
            quality: 1,
            flags: 8,
            buy_price: 35,
            sell_price: 7,
            inventory_type: 13,
            allowable_class: -1,
            allowable_race: -2,
            item_level: 1,
            required_level: 3,
            max_count: 4,
            stackable: 5,
            delay: 1600,
            damage: (1.5, 3.25, 6),
            block: 7,
            armor: 8,
            stats: vec![(7, 4), (6, 4)],
        }];
        w.objects = vec![ObjectTemplate {
            entry: 61,
            name: "Crate of Candles".into(),
            kind: 3,
            display: 259,
            faction: 35,
            flags: 4,
            size: 1.5,
            data: [57, 61, 0, -1, i32::MAX, i32::MIN, 6, -7, 8, 9],
        }];
        w.object_spawns = vec![ObjectSpawn {
            guid: 100,
            entry: 61,
            pos: [-8900.5, -120.25, 80.0],
            orientation: 0.5,
            respawn_min: 300,
            respawn_max: 360,
            area: 86,
            event: true,
        }];
        w.loot = vec![
            LootRow {
                kind: LootKind::Creature,
                table: 38,
                item: 752,
                chance: -80.0,
                group: 0,
                min_or_ref: 1,
                max_count: 1,
            },
            LootRow {
                kind: LootKind::Object,
                table: 61,
                item: 5,
                chance: 12.5,
                group: 2,
                min_or_ref: -100500,
                max_count: 3,
            },
            LootRow {
                kind: LootKind::Reference,
                table: 100500,
                item: 6,
                chance: 0.0,
                group: 1,
                min_or_ref: 2,
                max_count: 2,
            },
        ];
        w.vendors = vec![Vendor {
            npc: 152,
            item: 159,
            max_count: 5,
            incr_time: 3600,
        }];
        w.equipment = vec![Equipment {
            entry: 4,
            probability: 100,
            items: [1, 2, 3],
        }];
        w
    }

    #[test]
    fn write_then_parse_is_the_identity() {
        let w = sample();
        let text = write(&w);
        assert!(text.starts_with("wsworld 4\n"));
        assert_eq!(parse(&text).unwrap(), w);
    }

    #[test]
    fn every_record_kind_and_field_survives_a_round_trip() {
        let w = full();
        let text = write(&w);
        let back = parse(&text).unwrap();
        assert_eq!(back, w);
        // Floats come back bit for bit, not just nearly equal.
        assert_eq!(back.spawns[1].pos[0].to_bits(), 0.1f32.to_bits());
        assert_eq!(back.spawns[1].pos[2].to_bits(), f32::MAX.to_bits());
        // Every record kind is written.
        let kinds: std::collections::BTreeSet<_> = records(&text)
            .unwrap()
            .into_iter()
            .skip(1)
            .map(|r| r[0].clone().unwrap())
            .collect();
        let want: std::collections::BTreeSet<_> = [
            "area",
            "creature",
            "creaturemeta",
            "pool",
            "spawn",
            "waypoint",
            "quest",
            "start",
            "end",
            "item",
            "itemx",
            "object",
            "objspawn",
            "loot",
            "vendor",
            "equip",
            "qscript",
            "xp",
            "levelstat",
            "classstat",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(kinds, want);
    }

    #[test]
    fn writing_twice_gives_the_same_text() {
        assert_eq!(write(&sample()), write(&sample()));
        assert_eq!(write(&full()), write(&full()));
    }

    #[test]
    fn parse_then_write_gives_the_same_text() {
        let text = write(&full());
        assert_eq!(write(&parse(&text).unwrap()), text);
    }

    #[test]
    fn an_empty_world_is_just_the_header() {
        let text = write(&World::default());
        assert_eq!(text, "wsworld 4\n");
        assert_eq!(parse(&text).unwrap(), World::default());
    }

    #[test]
    fn the_line_layout_is_the_documented_one() {
        let text = write(&sample());
        let lines: Vec<&str> = text.split('\n').collect();
        assert_eq!(lines[0], "wsworld 4");
        assert_eq!(lines[1], "area\t9\tNorthshire Valley");
        // guid, entry, alt x4, pos x3, orientation, respawn min / max, wander, movement, area, event.
        assert_eq!(
            lines[2],
            "spawn\t80247\t103\t0\t0\t0\t0\t-9056.48\t-460.903\t72.8318\t1.5\t270\t270\t0\t0\t9\t0"
        );
        // guid, point, pos x3, orientation, wait in ms, script id, path id.
        let text = write(&full());
        assert!(
            text.contains(
                "\nwaypoint\t6732\t1\t-9638.02\t-3164.06\t49.1948\t3.11049\t5000\t7\t2\n"
            ),
            "{text}"
        );
        // The quest's text keeps its tab, newline and backslash as escapes; a list is a count then
        // its entries.
        assert!(
            text.contains("\tHello $N,\\\nit\\\ttabs\\\\slash\t"),
            "{text:?}"
        );
        // Up to `event`, then the fields wsworld 2 appended: max level, condition, three (faction, value)
        // pairs, the reward reputation (a count, then pairs) and the mail (template, delay, money).
        assert!(
            text.contains(
                "\t1\t752\t12\t1\t-6\t3\t0\t2\t2224\t1\t5580\t1\t0\t25\t355\t0\t0\t0\t0\t0\t0\t0\
                 \t0\t0\t0\t0\t0\t0\t0\t0\t1\t72\t100\t0\t0\t0\n"
            ),
            "{text:?}"
        );
        // An item extra and an xp row: entry, start quest, bonding / level, xp to the next.
        let text = write(&full());
        assert!(text.contains("\nitemx\t2092\t5805\t4\n"), "{text}");
        assert!(text.contains("\nxp\t2\t900\n"), "{text}");
    }

    /// A `wsworld 1` file, as `tools/export-mangos.ps1` + `import` wrote them before the quest engine:
    /// the records are `wsworld 2`'s without the appended fields and without the new kinds.
    fn without_the_wsworld_2_fields(mut w: World) -> World {
        for q in &mut w.quests {
            q.max_level = 0;
            q.required_condition = 0;
            q.rep_objective = (0, 0);
            q.rep_min = (0, 0);
            q.rep_max = (0, 0);
            q.reward_rep = vec![];
            q.reward_mail = (0, 0, 0);
        }
        for o in &mut w.objects {
            o.data[6..].fill(0);
        }
        w.scripts.clear();
        w.xp_for_level.clear();
        w.item_extras.clear();
        w.level_stats.clear();
        w.class_stats.clear();
        w.creature_meta.clear();
        w.spawn_pools.clear();
        for i in &mut w.items {
            i.stats.clear();
        }
        w
    }

    fn old_text_of(w: World) -> String {
        let text = write(&without_the_wsworld_2_fields(w));
        let mut out = String::from("wsworld 1\n");
        for rec in records(&text).unwrap().into_iter().skip(1) {
            let mut f: Vec<String> = rec.into_iter().map(|x| x.unwrap_or_default()).collect();
            match f[0].as_str() {
                // The appended fields of a quest without reward reputation: 8 numbers, the count 0, 3 numbers.
                "quest" => f.truncate(f.len() - 12),
                "object" => f.truncate(f.len() - 4),
                // The stats list wsworld 3 appended to an item without stats: its count, 0.
                "item" => f.truncate(f.len() - 1),
                _ => {}
            }
            out.push_str(&f.iter().map(|x| escape(x)).collect::<Vec<_>>().join("\t"));
            out.push('\n');
        }
        out
    }

    #[test]
    fn a_wsworld_1_file_still_reads_and_the_new_fields_are_zero() {
        let old = old_text_of(full());
        assert!(old.starts_with("wsworld 1\n") && !old.contains("\nxp\t"));
        let w = parse(&old).unwrap();
        let q = w.quests.iter().find(|q| q.id == 7).unwrap();
        assert!(q.event, "the old fields are read as before");
        assert_eq!(q.complete_script, 35);
        assert_eq!(
            (q.max_level, q.required_condition, q.rep_min, q.reward_mail),
            (0, 0, (0, 0), (0, 0, 0))
        );
        assert!(q.reward_rep.is_empty());
        assert_eq!(
            w.objects[0].data,
            [57, 61, 0, -1, i32::MAX, i32::MIN, 0, 0, 0, 0]
        );
        assert!(w.scripts.is_empty() && w.xp_for_level.is_empty() && w.item_extras.is_empty());
        // Everything else of the world is the same as the new one.
        assert_eq!(w, without_the_wsworld_2_fields(full()));
    }

    #[test]
    fn stats_and_the_level_tables_round_trip_in_the_current_version() {
        let w = full();
        let text = write(&w);
        assert!(text.starts_with("wsworld 4\n"), "{text}");
        assert!(
            text.contains("\nlevelstat\t1\t2\t24\t21\t23\t20\t20\n"),
            "{text}"
        );
        assert!(text.contains("\nclassstat\t1\t2\t29\t0\n"), "{text}");
        assert_eq!(parse(&text).unwrap(), w);
    }

    #[test]
    fn a_wsworld_2_file_still_loads_with_no_stats_and_no_tables() {
        let mut w = full();
        w.level_stats.clear();
        w.class_stats.clear();
        w.creature_meta.clear();
        w.spawn_pools.clear();
        for i in &mut w.items {
            i.stats.clear();
        }
        // The version 2 text: the same records, header 2, no stats list on the item, no new kinds.
        let text: String = write(&w)
            .replacen("wsworld 4", "wsworld 2", 1)
            .lines()
            .map(|l| {
                if l.starts_with("item\t") {
                    // drop the trailing "0" count the version 3 writer appends
                    l.rsplit_once('\t')
                        .map(|(head, _)| head)
                        .unwrap()
                        .to_string()
                } else {
                    l.to_string()
                }
            })
            .map(|l| l + "\n")
            .collect();
        let back = parse(&text).unwrap();
        assert!(back.level_stats.is_empty() && back.class_stats.is_empty());
        assert!(back.items.iter().all(|i| i.stats.is_empty()));
        assert_eq!(back, w);
    }

    #[test]
    fn a_wsworld_3_file_keeps_the_new_metadata_vectors_empty() {
        let mut world = full();
        world.creature_meta.clear();
        world.spawn_pools.clear();
        let old = write(&world).replacen("wsworld 4", "wsworld 3", 1);
        let back = parse(&old).unwrap();
        assert!(back.creature_meta.is_empty() && back.spawn_pools.is_empty());
        assert_eq!(back, world);
    }

    #[test]
    fn metadata_and_pools_round_trip_with_all_reactions() {
        let world = full();
        let text = write(&world);
        assert!(text.contains("\ncreaturemeta\t38\t10\t20\t30\t40\thostile\n"));
        assert!(text.contains("\ncreaturemeta\t721\t100\t0\t0\t0\tneutral\n"));
        assert!(text.contains("\ncreaturemeta\t197\t0\t50\t0\t50\tfriendly\n"));
        assert!(text.contains("\ncreaturemeta\t103\t0\t0\t0\t0\tunknown\n"));
        assert!(text.contains("\npool\t1270\t1\t2\t80247\t0\t6732\t0.1\n"));
        let back = parse(&text).unwrap();
        assert_eq!(back.creature_meta, world.creature_meta);
        assert_eq!(back.spawn_pools, world.spawn_pools);
        assert_eq!(
            back.spawn_pools[0].members[1].chance.to_bits(),
            0.1f32.to_bits()
        );
    }

    #[test]
    fn version_4_kinds_are_refused_in_every_older_version() {
        for version in 1..=3 {
            for record in ["creaturemeta\t38\t0\t0\t0\t0\tunknown", "pool\t1270\t1\t0"] {
                let err = parse(&format!("wsworld {version}\n{record}\n"))
                    .unwrap_err()
                    .to_string();
                assert!(err.contains("needs wsworld 4"), "{err}");
            }
        }
    }

    #[test]
    fn metadata_and_pool_validation_rejects_ambiguous_or_invalid_records() {
        for record in [
            "creaturemeta\t38\t0\t0\t0\t0\tangry",
            "creaturemeta\t38\t0\t0\t0\t0\tunknown\ncreaturemeta\t38\t1\t0\t0\t0\thostile",
            "pool\t1270\t0\t1\t11\t0",
            "pool\t1270\t1\t1\t11\tNaN",
            "pool\t1270\t1\t1\t11\tinf",
            "pool\t1270\t1\t1\t11\t-1",
            "pool\t1270\t1\t2\t11\t0\t11\t0",
            "pool\t1270\t1\t1\t11\t0\npool\t3200\t1\t1\t11\t0",
            "pool\t1270\t1\t0\npool\t1270\t1\t0",
            "pool\t1270\t1\t18446744073709551615",
        ] {
            assert!(
                parse(&format!("wsworld 4\n{record}\n")).is_err(),
                "{record}"
            );
        }
    }

    #[test]
    fn the_new_kinds_are_refused_in_an_older_file() {
        for line in [
            "levelstat\t1\t1\t23\t20\t22\t20\t20",
            "classstat\t1\t1\t20\t0",
        ] {
            let text = format!("wsworld 2\n{line}\n");
            let e = parse(&text).unwrap_err().to_string();
            assert!(e.contains("wsworld 3"), "{e}");
        }
    }

    #[test]
    fn a_wsworld_1_file_refuses_the_kinds_that_came_with_wsworld_2() {
        for line in ["itemx\t1\t2\t3", "xp\t1\t400"] {
            let e = parse(&format!("wsworld 1\n{line}\n"))
                .unwrap_err()
                .to_string();
            assert!(e.contains("wsworld 2"), "{e}");
        }
    }

    #[test]
    fn the_xp_levels_must_count_up_from_one() {
        assert_eq!(
            parse("wsworld 2\nxp\t1\t400\nxp\t2\t900\n")
                .unwrap()
                .xp_for_level,
            vec![400, 900]
        );
        for bad in [
            "xp\t2\t900\n",
            "xp\t1\t400\nxp\t1\t400\n",
            "xp\t1\t400\nxp\t3\t1\n",
        ] {
            let e = parse(&format!("wsworld 2\n{bad}")).unwrap_err().to_string();
            assert!(e.contains("xp") && e.contains("count up"), "{e}");
        }
    }

    #[test]
    fn an_unknown_version_or_kind_is_an_error() {
        let v = parse("wsworld 5\n").unwrap_err().to_string();
        assert!(v.contains("version") && v.contains("`5`"), "{v}");
        assert!(parse("wsworld 0\n").is_err());
        assert!(parse("wsworld 2\nbogus\t1\n")
            .unwrap_err()
            .to_string()
            .contains("bogus"));
        assert!(parse("wsworld 1\nbogus\t1\n")
            .unwrap_err()
            .to_string()
            .contains("bogus"));
    }

    #[test]
    fn a_file_that_is_not_a_manifest_is_an_error() {
        assert!(parse("").is_err());
        assert!(parse("creature\t1\n").is_err());
        assert!(parse("wsworld\n").is_err());
        assert!(parse("wsworld 1 2\n").is_err());
    }

    #[test]
    fn a_record_with_too_few_or_too_many_fields_names_the_kind() {
        let few = parse("wsworld 1\nvendor\t1\t2\t3\n")
            .unwrap_err()
            .to_string();
        assert!(few.contains("vendor") && few.contains("few"), "{few}");
        let many = parse("wsworld 1\nvendor\t1\t2\t3\t4\t5\n")
            .unwrap_err()
            .to_string();
        assert!(many.contains("vendor") && many.contains("extra"), "{many}");
        let bad = parse("wsworld 1\nvendor\t1\t2\tx\t4\n")
            .unwrap_err()
            .to_string();
        assert!(bad.contains("vendor") && bad.contains("`x`"), "{bad}");
        let flag = parse("wsworld 1\nstart\t1\t2\t7\n")
            .unwrap_err()
            .to_string();
        assert!(flag.contains("start") && flag.contains("`7`"), "{flag}");
        let loot = parse("wsworld 1\nloot\tmonster\t1\t2\t3\t4\t5\t6\n")
            .unwrap_err()
            .to_string();
        assert!(loot.contains("loot") && loot.contains("monster"), "{loot}");
    }
}
