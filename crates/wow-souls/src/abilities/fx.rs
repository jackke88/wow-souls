//! What the abilities look and sound like, all WoW's own: the models and sound kits of each spell's
//! `SpellVisual` (read with `benilla-extract spellvis <spell>`):
//!
//! - Charge (100): the trail model on the warrior while he rushes (`attach 0x22`), a dust cloud where he lands
//!   (`0x13`), sound kit 3330.
//! - Thunder Clap (6343): lightning in both hands (`0x15`, `0x16`), the clap's own ground burst, sound kit 3090;
//!   on each enemy the lightning impact (`0x22`, sound kit 1506) and the slow's mark (`0x13`).
//! - Battle Shout (6673): the shout's glow (`0x13`), sound kit 3113.
//!
//! The models play as the loot sparkles do ([`crate::loot::vfx::effect`]): the first sequence once, built
//! as the client builds a spell-visual effect (its batches with their own alpha and colour clocks, a
//! flat ground quad as a decal that drapes the terrain, its emitters and ribbons), the geometry gone
//! when the sequence ends and the model a second after. Battle Shout is six such ground quads: red
//! crescents that fly out to both sides of the warrior while they grow.

use benilla_assets::{m2_url, M2Model};
use bevy::prelude::*;

use crate::audio::Audio;
use crate::loot::vfx;
use crate::settle::Phase;

/// The sound kits (`SoundEntries` ids) of the spells' visual kits.
pub const SOUND_CHARGE: u32 = 3330;
pub const SOUND_CLAP: u32 = 3090;
pub const SOUND_CLAP_HIT: u32 = 1506;
pub const SOUND_SHOUT: u32 = 3113;
/// How loud they are against the fight's other sounds.
const SCALE: f32 = 0.9;

/// A spell model and the attachment point it rides (`None`: the body's origin, at the feet).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    ChargeTrail,
    ChargeDust,
    ClapBase,
    ClapHand(u16),
    ClapImpact,
    Slow,
    Shout,
}

impl Kind {
    /// The model in the client's archives (`.mdx` names, read as `.m2`).
    pub fn path(self) -> &'static str {
        match self {
            Kind::ChargeTrail => "Spells\\ChargeTrail.mdx",
            Kind::ChargeDust => "Spells\\DustCloud_Land.mdx",
            Kind::ClapBase => "Spells\\ThunderClap_Cast_Base.mdx",
            Kind::ClapHand(_) => "Spells\\Lightning_Cast_Hand.mdx",
            Kind::ClapImpact => "Spells\\LightningBolt_Impact_Chest.mdx",
            Kind::Slow => "Spells\\Slow_Impact_Base.mdx",
            Kind::Shout => "Spells\\BattleShout_Cast_Base.mdx",
        }
    }

    /// The attachment id the spell's visual kit hangs it on.
    pub fn attach(self) -> Option<u16> {
        match self {
            Kind::ChargeTrail | Kind::ClapImpact => Some(0x22),
            Kind::ChargeDust | Kind::Slow | Kind::Shout => Some(0x13),
            Kind::ClapHand(id) => Some(id),
            Kind::ClapBase => None,
        }
    }

    const ALL: [Kind; 8] = [
        Kind::ChargeTrail,
        Kind::ChargeDust,
        Kind::ClapBase,
        Kind::ClapHand(0x15),
        Kind::ClapHand(0x16),
        Kind::ClapImpact,
        Kind::Slow,
        Kind::Shout,
    ];

    fn name(self) -> &'static str {
        match self {
            Kind::ChargeTrail => "WoW charge trail",
            Kind::ChargeDust => "WoW charge dust",
            Kind::ClapBase => "WoW thunder clap",
            Kind::ClapHand(_) => "WoW thunder clap hand",
            Kind::ClapImpact => "WoW thunder clap impact",
            Kind::Slow => "WoW slow mark",
            Kind::Shout => "WoW battle shout",
        }
    }
}

/// The loaded models, by path.
#[derive(Resource, Default)]
pub struct Fx(std::collections::BTreeMap<&'static str, Handle<M2Model>>);

fn load(mut commands: Commands, assets: Res<AssetServer>) {
    let models = Kind::ALL
        .iter()
        .map(|k| (k.path(), assets.load(m2_url(k.path()))))
        .collect();
    commands.insert_resource(Fx(models));
}

/// Puts `kind` on `owner`; `None` when its model was not loaded (no client data).
pub fn spawn(commands: &mut Commands, fx: &Fx, kind: Kind, owner: Entity) -> Option<Entity> {
    let model = fx.0.get(kind.path())?.clone();
    Some(vfx::effect(
        commands,
        kind.name(),
        owner,
        model,
        kind.attach(),
        true,
    ))
}

/// Plays sound kit `kit` at the player, not positioned.
pub fn sound(audio: Option<&mut Audio>, kit: u32, what: &str) {
    if let Some(audio) = audio {
        audio.play_kit(kit, None, SCALE, what);
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Fx>()
        .add_systems(OnEnter(Phase::Playing), load);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_model_and_sound_of_the_abilities_is_in_the_install() {
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        for kind in Kind::ALL {
            // The client names its effect models `.mdx` and ships them as `.m2`.
            let path = kind.path().replace(".mdx", ".m2");
            chain
                .read_file(&path)
                .unwrap_or_else(|e| panic!("{path}: {e:#}"));
        }
        let kits = benilla_formats::load_sound_kit_catalog(&mut chain).unwrap();
        for kit in [SOUND_CHARGE, SOUND_CLAP, SOUND_CLAP_HIT, SOUND_SHOUT] {
            let k = kits.get(kit).unwrap_or_else(|| panic!("sound kit {kit}"));
            assert!(!k.files.is_empty(), "sound kit {kit} has files");
            for (file, _) in &k.files {
                let bytes = chain
                    .read_file(file)
                    .unwrap_or_else(|e| panic!("{file}: {e:#}"));
                crate::sfx::decode(bytes, 48_000).unwrap_or_else(|e| panic!("{file}: {e:#}"));
            }
        }
    }

    #[test]
    fn a_spell_model_rides_the_attachment_its_visual_kit_names() {
        assert_eq!(Kind::ChargeTrail.attach(), Some(0x22));
        assert_eq!(Kind::ChargeDust.attach(), Some(0x13));
        assert_eq!(Kind::ClapHand(0x15).attach(), Some(0x15));
        assert_eq!(
            Kind::ClapBase.attach(),
            None,
            "the clap's burst is on the ground"
        );
    }
}
