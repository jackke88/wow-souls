//! Warrior builds. Stable save IDs belong to the rules, independently of the menu and combat.
//! One point at each level 10–60; tree gates count points in that tree's earlier tiers.

pub const WARRIOR: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tree {
    Arms,
    Fury,
    Protection,
}

impl Tree {
    pub const ALL: [Self; 3] = [Self::Arms, Self::Fury, Self::Protection];

    pub fn name(self) -> &'static str {
        match self {
            Self::Arms => "Arms",
            Self::Fury => "Fury",
            Self::Protection => "Protection",
        }
    }

    pub fn talents(self) -> impl Iterator<Item = TalentId> {
        TalentId::ALL
            .into_iter()
            .filter(move |id| id.tree() == self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TalentId {
    TacticalMastery,
    ImprovedCharge,
    HeavyHand,
    Impale,
    SecondWind,
    Bloodletting,
    MortalStrike,
    UnbridledWrath,
    Flurry,
    Cruelty,
    BloodCraze,
    BattleTrance,
    Berserker,
    Bloodthirst,
    ShieldSpecialization,
    Deflection,
    ShieldDiscipline,
    Revenge,
    Bulwark,
    Concussion,
    ShieldWall,
}

impl TalentId {
    pub const ALL: [Self; 21] = [
        Self::TacticalMastery,
        Self::ImprovedCharge,
        Self::HeavyHand,
        Self::Impale,
        Self::SecondWind,
        Self::Bloodletting,
        Self::MortalStrike,
        Self::UnbridledWrath,
        Self::Flurry,
        Self::Cruelty,
        Self::BloodCraze,
        Self::BattleTrance,
        Self::Berserker,
        Self::Bloodthirst,
        Self::ShieldSpecialization,
        Self::Deflection,
        Self::ShieldDiscipline,
        Self::Revenge,
        Self::Bulwark,
        Self::Concussion,
        Self::ShieldWall,
    ];

    pub fn tree(self) -> Tree {
        match self as usize / 7 {
            0 => Tree::Arms,
            1 => Tree::Fury,
            _ => Tree::Protection,
        }
    }

    pub fn gate(self) -> u8 {
        [0, 0, 5, 5, 10, 15, 25][self as usize % 7]
    }

    pub fn max_rank(self) -> u8 {
        if self as usize % 7 == 6 {
            1
        } else {
            5
        }
    }

    pub fn id(self) -> &'static str {
        [
            "tactical-mastery",
            "improved-charge",
            "heavy-hand",
            "impale",
            "second-wind",
            "bloodletting",
            "mortal-strike",
            "unbridled-wrath",
            "flurry",
            "cruelty",
            "blood-craze",
            "battle-trance",
            "berserker",
            "bloodthirst",
            "shield-specialization",
            "deflection",
            "shield-discipline",
            "revenge",
            "bulwark",
            "concussion",
            "shield-wall",
        ][self as usize]
    }

    pub fn from_id(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.id() == value)
    }

    pub fn name(self) -> &'static str {
        [
            "Tactical Mastery",
            "Improved Charge",
            "Heavy Hand",
            "Impale",
            "Second Wind",
            "Bloodletting",
            "Mortal Strike",
            "Unbridled Wrath",
            "Flurry",
            "Cruelty",
            "Blood Craze",
            "Battle Trance",
            "Berserker",
            "Bloodthirst",
            "Shield Specialization",
            "Deflection",
            "Shield Discipline",
            "Revenge",
            "Bulwark",
            "Concussion",
            "Shield Wall",
        ][self as usize]
    }

    /// Per-rank effects, also used by the in-game cards.
    pub fn description(self) -> &'static str {
        match self {
            Self::TacticalMastery => "Rage decays 10% slower per rank outside combat.",
            Self::ImprovedCharge => "Charge cooldown is reduced by 0.5 seconds per rank.",
            Self::HeavyHand => "Heavy attacks deal 8% more poise damage per rank.",
            Self::Impale => "Heavy attacks deal 3% more damage per rank.",
            Self::SecondWind => "A perfect parry restores 2 stamina per rank.",
            Self::Bloodletting => "Heavy hits bleed for 5% of damage dealt per rank over 4 seconds. Refreshes; does not stack.",
            Self::MortalStrike => "Unlock Mortal Strike: a committed heavy strike that spends rage. Automatically added to your ability bar.",
            Self::UnbridledWrath => "Weapon hits generate 8% more rage per rank.",
            Self::Flurry => "Landing a weapon swing refunds 1 stamina per rank, once per swing.",
            Self::Cruelty => "Chained follow-up attacks deal 2% more damage per rank.",
            Self::BloodCraze => "A melee kill restores 0.5% maximum health per rank, once per swing.",
            Self::BattleTrance => "At 50 rage or more, deal 2% more weapon damage per rank.",
            Self::Berserker => "At 50% health or less, deal 2% more weapon damage per rank.",
            Self::Bloodthirst => "Unlock Bloodthirst: spend rage on a swift melee attack that restores health on hit. Automatically added to your ability bar.",
            Self::ShieldSpecialization => "Blocking costs 4% less stamina per rank.",
            Self::Deflection => "Parrying costs 0.8 less stamina per rank.",
            Self::ShieldDiscipline => "A perfect block generates 1 additional rage per rank.",
            Self::Revenge => "After a perfect block or parry, your next melee hit on the attacker within 3 seconds deals 4% more damage per rank.",
            Self::Bulwark => "Normal blocks take 8% less chip damage per rank.",
            Self::Concussion => "After a perfect block or parry, your next melee hit on the attacker within 3 seconds deals 8% more poise damage per rank.",
            Self::ShieldWall => "Unlock Shield Wall: spend rage for a short defensive stand. Automatically added to your ability bar.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Build {
    ranks: [u8; 21],
}

impl Build {
    pub fn rank(&self, id: TalentId) -> u8 {
        self.ranks[id as usize]
    }

    pub fn spent(&self) -> u8 {
        self.ranks.iter().sum()
    }

    pub fn points_in(&self, tree: Tree) -> u8 {
        tree.talents().map(|id| self.rank(id)).sum()
    }

    pub fn budget(level: u8) -> u8 {
        level.min(60).saturating_sub(9)
    }

    pub fn available(&self, level: u8) -> u8 {
        Self::budget(level).saturating_sub(self.spent())
    }

    pub fn signature(&self) -> Option<u32> {
        [
            (TalentId::MortalStrike, 12294),
            (TalentId::Bloodthirst, 23881),
            (TalentId::ShieldWall, 871),
        ]
        .into_iter()
        .find_map(|(id, spell)| (self.rank(id) > 0).then_some(spell))
    }

    pub fn spend(&mut self, id: TalentId, level: u8) -> Result<(), String> {
        self.validate(WARRIOR, level)?;
        if self.rank(id) >= id.max_rank() {
            return Err("This talent is already at maximum rank.".into());
        }
        if self.available(level) == 0 {
            return Err("No talent points available. Gain a level (10–60).".into());
        }
        if self.points_in(id.tree()) < id.gate() {
            return Err(format!(
                "Spend {} points in {} first.",
                id.gate(),
                id.tree().name()
            ));
        }
        self.ranks[id as usize] += 1;
        Ok(())
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn validate(&self, class: u8, level: u8) -> Result<(), String> {
        if class != WARRIOR {
            return Err("Warrior talents require a Warrior character.".into());
        }
        if self.ranks.iter().any(|&r| r > 5) {
            return Err("Invalid talent rank.".into());
        }
        if self.spent() > Self::budget(level) {
            return Err(format!("Talent points exceed the budget at level {level}."));
        }
        for id in TalentId::ALL {
            let rank = self.rank(id);
            if rank > id.max_rank() {
                return Err(format!("{} exceeds its maximum rank.", id.name()));
            }
            let below: u8 = id
                .tree()
                .talents()
                .filter(|other| other.gate() < id.gate())
                .map(|other| self.rank(other))
                .sum();
            if rank > 0 && id.gate() > below {
                return Err(format!(
                    "{} requires {} points in earlier {} tiers.",
                    id.name(),
                    id.gate(),
                    id.tree().name()
                ));
            }
        }
        Ok(())
    }

    pub fn text(&self) -> String {
        let mut text = "wstalents 1\nclass 1\n".to_string();
        for id in TalentId::ALL {
            if self.rank(id) > 0 {
                text.push_str(&format!("{} {}\n", id.id(), self.rank(id)));
            }
        }
        text
    }

    pub fn parse(text: &str, level: u8) -> Result<Self, String> {
        let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
        if lines.next() != Some("wstalents 1") {
            return Err("Expected wstalents 1.".into());
        }
        if lines.next() != Some("class 1") {
            return Err("Expected Warrior class 1.".into());
        }
        let mut build = Self::default();
        let mut seen = [false; 21];
        for line in lines {
            let parts: Vec<_> = line.split_whitespace().collect();
            let [name, rank] = parts.as_slice() else {
                return Err(format!("Invalid talent line: {line}"));
            };
            let id = TalentId::from_id(name).ok_or_else(|| format!("Unknown talent: {name}"))?;
            if seen[id as usize] {
                return Err(format!("Duplicate talent: {name}"));
            }
            seen[id as usize] = true;
            build.ranks[id as usize] = rank.parse().map_err(|_| format!("Invalid rank: {line}"))?;
        }
        build.validate(WARRIOR, level)?;
        Ok(build)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn five(build: &mut Build, id: TalentId) {
        for _ in 0..5 {
            build.spend(id, 60).unwrap();
        }
    }

    #[test]
    fn points_start_at_ten_and_stop_at_sixty() {
        for (level, budget) in [(0, 0), (9, 0), (10, 1), (11, 2), (60, 51), (255, 51)] {
            assert_eq!(Build::budget(level), budget);
        }
        let mut build = Build::default();
        assert!(build.spend(TalentId::Flurry, 9).is_err());
        build.spend(TalentId::Flurry, 10).unwrap();
        assert!(build.spend(TalentId::TacticalMastery, 10).is_err());
    }

    #[test]
    fn tiers_allow_hybrids_but_two_signatures_exceed_the_budget() {
        let mut build = Build::default();
        assert!(build.spend(TalentId::MortalStrike, 60).is_err());
        for id in [
            TalentId::TacticalMastery,
            TalentId::ImprovedCharge,
            TalentId::HeavyHand,
            TalentId::Impale,
            TalentId::SecondWind,
        ] {
            five(&mut build, id);
        }
        build.spend(TalentId::MortalStrike, 60).unwrap();
        assert_eq!(build.signature(), Some(12294));
        assert!(build.spend(TalentId::MortalStrike, 60).is_err());
        for id in [
            TalentId::UnbridledWrath,
            TalentId::Flurry,
            TalentId::Cruelty,
            TalentId::BloodCraze,
            TalentId::BattleTrance,
        ] {
            five(&mut build, id);
        }
        assert_eq!(build.spent(), 51);
        assert!(build.spend(TalentId::Bloodthirst, 60).is_err());
        assert_eq!(Build::parse(&build.text(), 60), Ok(build.clone()));
        assert!(Build::parse(&build.text(), 59).is_err());
        build.reset();
        assert_eq!(build.available(60), 51);
    }

    #[test]
    fn saves_reject_corruption_and_self_funding_tiers() {
        for text in [
            "",
            "wstalents 2\nclass 1",
            "wstalents 1\nclass 8",
            "wstalents 1\nclass 1\nunknown 1",
            "wstalents 1\nclass 1\nflurry 6",
            "wstalents 1\nclass 1\nflurry -1",
            "wstalents 1\nclass 1\nflurry 255",
            "wstalents 1\nclass 1\nflurry 0\nflurry 1",
            "wstalents 1\nclass 1\nheavy-hand 5\nimpale 5",
            "wstalents 1\nclass 1\nshield-wall 2",
        ] {
            assert!(Build::parse(text, 60).is_err(), "{text}");
        }
        assert!(Build::default().validate(8, 60).is_err());
    }

    #[test]
    fn every_node_has_a_unique_stable_id_and_each_tree_has_thirty_one_ranks() {
        for (n, id) in TalentId::ALL.into_iter().enumerate() {
            assert_eq!(TalentId::from_id(id.id()), Some(id));
            assert!(!TalentId::ALL[..n].iter().any(|other| other.id() == id.id()));
        }
        for tree in Tree::ALL {
            assert_eq!(tree.talents().map(TalentId::max_rank).sum::<u8>(), 31);
        }
    }
}
