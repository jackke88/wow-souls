//! Reproducible Westfall NPC, quest and animation worklist from packaged data and the client.
use crate::world::Kind;

fn missing_motion(body: &str) -> &'static str {
    let body = body.to_ascii_lowercase();
    if body.contains("golemharvest") {
        "Tung idle/gång, vändning, armsvep/skördehugg, laddat dubbelhugg, träffreaktioner och kollaps"
    } else if body.contains("carrionbird") {
        "Ving-/svävloop, flykt/start/landning, dykning, näbbhugg, klor, träffreaktion och fall/död"
    } else if body.contains("crab") {
        "Idle, sidogång/löpning, vändning, vänster/höger klonyp, dubbelnyp, rekyl och död"
    } else if body.contains("airelemental") {
        "Svävande virvel, förflyttning/vändning, vindstöt, utfall, träffreaktion och upplösning"
    } else if body.contains("gnoll") {
        "Egen benmappning för modellen, hållning/gång/löpning, vapen- eller stavattacker, reaktioner och död"
    } else if body.contains("scourge") {
        "Odöd kroppsrigg: hållning/gång/löpning, närstrid, träffreaktioner och död"
    } else {
        "Idle, gång/löpning, vändning, attacker, träffreaktioner och död"
    }
}
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fmt::Write;

pub fn run() -> Result<()> {
    let load = |name| -> Result<wow_world::model::World> {
        wow_world::manifest::parse(&std::fs::read_to_string(wow_world::project_path(
            &format!("data/world/{name}.txt"),
        ))?)
    };
    let westfall = load("westfall")?;
    let combined = load("elwynn-westfall")?;
    let book = wow_world::quest::QuestBook::new(&combined);
    let data =
        benilla_formats::wow_data().context("set WOW_DATA to the WoW 1.12 Data directory")?;
    let mut chain = benilla_formats::open_chain(&data)?;
    let models = benilla_formats::load_creature_catalog(&mut chain)?;
    let mut npc_tsv = String::from("entry\tname\tspawn_sites_including_alternates\tcategory\tmodels\tproject_kits\tanimation_work\n");
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut native_families: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut local_count = 0;
    for c in &westfall.creatures {
        let count = westfall
            .spawns
            .iter()
            .filter(|s| s.entry == c.entry || s.alt.contains(&c.entry))
            .count();
        local_count += usize::from(count > 0);
        let meta = westfall.creature_meta.iter().find(|m| m.entry == c.entry);
        let kind = crate::world::kind_with_meta(c, meta);
        let body: Vec<_> = c
            .display
            .iter()
            .filter(|d| **d != 0)
            .filter_map(|d| models.model(*d))
            .collect();
        let paths: Vec<_> = body.iter().map(|m| m.model_path.clone()).collect();
        let mut kits: Vec<_> = body
            .iter()
            .map(|m| crate::creature_motion::base(&m.model_path).map_or("native_wow", |s| s.kit))
            .collect();
        kits.sort_unstable();
        kits.dedup();
        let hostile = matches!(kind, Kind::Foe(_) | Kind::ImportedFoe);
        let new_body = hostile && kits.contains(&"native_wow");
        let (category, work) = if count == 0 {
            ("Utanför Westfall – uppdragsrelation", "Ingen lokal spawn")
        } else if c.entry == crate::friendly::SPIRIT_HEALER {
            ("Gravplats", "Ersatt av gravsten; ingen ny NPC-animation")
        } else if matches!(c.entry, 467 | 6182 | 620) {
            (
                "Eskort och händelser",
                "WoW gång/strid finns; eskort- och händelsesekvenser behöver kopplas",
            )
        } else if new_body {
            ("P1 – ny kroppsrigg", "Idle, gång/löpning, vändning, attacker, träffreaktioner och död; kroppens naturliga träffdelar")
        } else if hostile && matches!(c.entry, 589 | 450 | 453 | 501 | 1065 | 456 | 517) {
            ("P2 – kastare", "Kompatibel kroppsrigg finns; besvärjelser, kastsekvenser och projektilkoppling behöver färdigställas")
        } else if hostile && (c.rank != 0 || matches!(c.entry, 391 | 7053 | 506 | 1424)) {
            (
                "P2 – namngiven fiende",
                "Basrigg finns; unika attacker och encounter-sekvenser återstår att utforma",
            )
        } else if hostile {
            (
                "Återanvändbar stridsrigg",
                "Befintliga rörelser; familjens variation och balans behöver speltestas",
            )
        } else if kind == Kind::Critter {
            ("Smådjur", "WoW-animationer räcker")
        } else {
            (
                "Vänlig NPC",
                "WoW idle/gång räcker; inga nya stridsanimationer planerade",
            )
        };
        if new_body && count > 0 {
            for model in &body {
                if crate::creature_motion::base(&model.model_path).is_none() {
                    native_families
                        .entry(model.model_path.clone())
                        .or_default()
                        .push(format!("{} ({})", c.name, c.entry));
                }
            }
        }
        groups
            .entry(category.to_string())
            .or_default()
            .push(format!("{} ({}, {} platser)", c.name, c.entry, count));
        writeln!(
            npc_tsv,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            c.entry,
            c.name,
            count,
            category,
            paths.join("; "),
            kits.join("; "),
            work
        )?;
        if count > 0 && meta.and_then(|m| m.reaction).is_none() {
            anyhow::bail!("unresolved reaction for {}", c.entry);
        }
        if count > 0 && body.is_empty() {
            anyhow::bail!("missing model for {}", c.entry);
        }
    }
    let mut doc = format!("# Westfall: NPC:er, uppdrag och animationsarbete\n\nGenererad från projektets vMaNGOS-export för 1.12 och spelarens WoW-klient. Kör wow-souls --audit-westfall med WOW_DATA för att uppdatera rapporten.\n\n{} spawnplatser, {} lokala NPC-/fiendetyper (inklusive alternativa entries), {} mallar inklusive externa uppdragsrelationer, {} uppdrag och {} patrullpunkter. Sammanfogad värld: {} platser och {} uppdrag.\n\nDatabasens säsongsevent och NYI-poster ingår inte i basvärlden. Deadmines-instansen är en separat karta och ingår inte i Westfalls utomhuspopulation. Täthet och rare-pooler väljer aktiva platser ur hela importen enligt spelets befintliga regler. Spirit Healer representeras av gravstenen.\n\nAlla importerade uppdrag finns i paketet. Det betyder inte att eskorter, stavmål eller serverskript är färdigimplementerade. Kolumnen Händelsearbete nedan anger vad som behöver kopplas; ingen sådan sekvens räknas som verifierad av denna import. Nya fienders grundläggande strid och nivåskalning är provisoriska.\n\n", westfall.spawns.len(), local_count, westfall.creatures.len(), westfall.quests.len(), westfall.waypoints.len(), combined.spawns.len(), combined.quests.len());
    doc += "## Nya animationspaket per kropp\n\nDessa visas med klientens egna M2-animationer. Nytt paket: idle, gång/löpning, vändning, attacker, träffreaktioner och död. Käke/näbb, klor, armar eller virvel behöver egna träffbanor.\n\n| WoW-kropp | Alla berörda NPC:er | Nya animationer |\n|---|---|---|\n";
    for (body, mut names) in native_families {
        names.sort();
        names.dedup();
        writeln!(
            doc,
            "| {body} | {} | {} |",
            names.join(", "),
            missing_motion(&body)
        )?;
    }
    doc += "\n## Samtliga NPC:er, kategoriserade\n\nFullständiga modellvägar och kit finns i [WESTFALL-NPCS.tsv](WESTFALL-NPCS.tsv). Antalet platser inkluderar alternativa entries; de aktiveras enligt databasens pooler.\n\n";
    for (category, names) in groups {
        writeln!(doc, "### {category}\n\n{}\n", names.join(", "))?;
    }
    doc += "## Samtliga uppdrag\n\nGivare och avslutare anges med databas-ID (NPC eller objekt). Kedjor över zongränser kan kräva ytterligare områden. Fullständig kedjeanalys finns i [WESTFALL-QUESTS.tsv](WESTFALL-QUESTS.tsv).\n\n| ID | Uppdrag | Nivå | Givare | Avslutare | Händelsearbete |\n|---|---|---:|---|---|---|\n";
    let mut quest_tsv = String::from("id\ttitle\tlevel\tgivers\tenders\twarrior_chain_reachable\tmage_chain_reachable\trogue_chain_reachable\texternal_prerequisites\tevent_work\n");
    for q in &westfall.quests {
        let relations = |rows: &[wow_world::model::Relation]| {
            rows.iter()
                .filter(|r| r.quest == q.id)
                .map(|r| format!("{}{}", if r.object { "obj:" } else { "npc:" }, r.npc))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut work = Vec::new();
        if q.start_script != 0 || q.complete_script != 0 {
            work.push("serverskript");
        }
        if q.special_flags & 2 != 0 {
            work.push("händelse/eskort");
        }
        if q.req_spell_cast.iter().any(|s| *s != 0) {
            work.push("stavmål");
        }
        if q.limit_time > 0 {
            work.push("tidsgräns");
        }
        if q.required_condition != 0 {
            work.push("databasvillkor");
        }
        if !book.giver_spawned(q.id) {
            work.push("extern givare/itemstart");
        }
        if !book.ender_spawned(q.id) {
            work.push("extern avslutare");
        }
        let pending = if work.is_empty() {
            "Standardmål – inte slutspeltestat".into()
        } else {
            work.join(", ")
        };
        let givers = relations(&westfall.starts);
        let enders = relations(&westfall.ends);
        let external = book
            .prev_quests(q.id)
            .iter()
            .filter(|id| !combined.quests.iter().any(|q| q.id == id.unsigned_abs()))
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            doc,
            "| {} | {} | {} | {} | {} | {} |",
            q.id,
            q.title.replace('|', "/"),
            q.level,
            givers,
            enders,
            pending
        )?;
        writeln!(
            quest_tsv,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            q.id,
            q.title,
            q.level,
            givers,
            enders,
            book.playable_for(q.id, wow_world::quest::Class::Warrior),
            book.playable_for(q.id, wow_world::quest::Class::Mage),
            book.playable_for(q.id, wow_world::quest::Class::Rogue),
            external,
            pending
        )?;
    }
    let out = wow_world::project_path("docs");
    std::fs::create_dir_all(&out)?;
    std::fs::write(out.join("WESTFALL.md"), doc)?;
    std::fs::write(out.join("WESTFALL-NPCS.tsv"), npc_tsv)?;
    std::fs::write(out.join("WESTFALL-QUESTS.tsv"), quest_tsv)?;
    println!(
        "Westfall audit: {} NPC templates, {} quests -> {}",
        westfall.creatures.len(),
        westfall.quests.len(),
        out.display()
    );
    Ok(())
}
