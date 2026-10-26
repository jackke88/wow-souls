//! Database creatures use a compatible project rig when one exists, otherwise native M2 motion.
//! The fallback is a basic melee opponent, not a finished family or scripted boss moveset.
use crate::species::{self, SpeciesDef};
use benilla_formats::CreatureModel;
use wow_world::model::Creature;

/// Exact model families only: never stretch a humanoid kit onto a new creature body.
pub fn base(model: &str) -> Option<SpeciesDef> {
    let path = model.replace('\\', "/").to_ascii_lowercase();
    Some(
        match path.trim_end_matches(".mdx").trim_end_matches(".m2") {
            "creature/wolf/wolf" => species::WOLF,
            "creature/direwolf/direwolf" => species::PROWLER,
            "creature/boar/boar" => species::STONETUSK,
            "creature/gnoll/gnoll" => species::OUTRUNNER,
            "creature/murloc/murloc" => species::MURLOC,
            "creature/kobold/kobold" => species::KOBOLD,
            "character/human/male/humanmale" => species::BANDIT,
            "character/human/female/humanfemale" => SpeciesDef {
                kit: "human_bandit_female",
                ..species::BANDIT
            },
            _ => return None,
        },
    )
}

pub fn definition(
    c: &Creature,
    display: u32,
    model: &CreatureModel,
    weapon: Option<u32>,
) -> SpeciesDef {
    let compatible = base(&model.model_path);
    let custom = compatible.is_some();
    let mut def = compatible.unwrap_or(species::KOBOLD);
    def.name = Box::leak(
        format!(
            "creature_{}_{}",
            c.entry,
            c.name
                .to_ascii_lowercase()
                .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        )
        .into_boxed_str(),
    );
    def.title = Box::leak(c.name.clone().into_boxed_str());
    def.display = display;
    def.look = None;
    def.weapon = weapon;
    def.offhand_weapon = None;
    def.level = c.level_max.clamp(1, 255) as u8;
    // Provisional combat tuning; DB identity, display, faction and placement remain authoritative.
    // Level-1 units like the hand tables (60-120 health, 0.7-1.0 damage): [`SpeciesDef::max_health`] and
    // [`SpeciesDef::blow_scale`] carry them to the creature's level.
    def.health = (45.0 + c.level_max as f32 * 6.0) * c.health_multiplier.max(0.1);
    def.damage_scale = 0.9 * c.damage_multiplier.max(0.1);
    def.rank = if matches!(c.rank, 2 | 4) {
        species::Rank::Rare
    } else {
        species::Rank::Normal
    };
    def.phase_two = None;
    def.size = c
        .display
        .iter()
        .position(|d| *d == display)
        .map(|i| c.scale[i])
        .filter(|s| *s > 0.0)
        .unwrap_or(1.0);
    if !custom {
        def.kit = "native_wow";
        def.behavior = crate::behavior::Role::DefiasBrawler;
        def.collision = (
            model.collision_height.max(0.5),
            (model.collision_height * 0.24).clamp(0.2, 0.9),
        );
        def.stop_distance = crate::npc::STOP_DISTANCE;
        def.poise = 0.0;
        def.chase_speed = (crate::motor::RUN_SPEED * c.speed_run).clamp(2.5, 7.0);
    }
    def
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_bodies_never_receive_a_humanoid_or_quadruped_kit() {
        for name in ["HarvestGolem", "Vulture", "Crab", "AirElemental"] {
            assert!(base(&format!("Creature\\{name}\\{name}.m2")).is_none());
        }
        assert_eq!(base("Creature\\Gnoll\\Gnoll.mdx").unwrap().kit, "gnoll");
        assert_eq!(
            base("Character\\Human\\Female\\HumanFemale.m2")
                .unwrap()
                .kit,
            "human_bandit_female"
        );
    }
}
