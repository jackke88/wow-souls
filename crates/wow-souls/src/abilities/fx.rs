//! What the abilities look and sound like, all WoW's own: the models and sound kits of each spell's
//! `SpellVisual` (read with `benilla-extract spellvis <spell>`):
//!
//! - Charge (100): the trail model on the warrior while he rushes (`attach 0x22`), a dust cloud where he lands
//!   (`0x13`), sound kit 3330.
//! - Thunder Clap (6343): lightning in both hands (`0x15`, `0x16`), the clap's own ground burst, sound kit 3090;
//!   on each enemy the lightning impact (`0x22`, sound kit 1506) and the slow's mark (`0x13`).
//! - Battle Shout (6673): the shout's glow (`0x13`), sound kit 3113.
//! - The Mage's (read with `spellvis 133 / 116 / 2136 / 168`): Fireball 133 (visual 67: fire in both hands
//!   winding up and releasing, the missile, the impact on the chest), Frostbolt 116 (visual 13: frost in
//!   the hands, the missile, the impact and the mark on the head), Fire Blast 2136 (visual 143: the cast
//!   hands and the impact) and Frost Armor 168 (visual 124: frost in the hands, the glow on the head).
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
/// The Mage's (the spells' visual kits: fire 702 / 1484 / 1507, frost 20 / 2559 / 7, Frost Armor 1456).
pub const SOUND_FIRE_PRECAST: u32 = 702;
pub const SOUND_FIRE_CAST: u32 = 1484;
pub const SOUND_FIRE_IMPACT: u32 = 1507;
pub const SOUND_ICE_PRECAST: u32 = 20;
pub const SOUND_ICE_CAST: u32 = 2559;
pub const SOUND_ICE_IMPACT: u32 = 7;
pub const SOUND_FROST_ARMOR: u32 = 1456;
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
    /// Fireball's missile (its own body, no attachment).
    FireballMissile,
    /// The fire glow in a hand while a fire spell winds up (`0x15` / `0x16`).
    FirePrecastHand(u16),
    /// The fire in a hand as the spell leaves it.
    FireHand(u16),
    /// Fireball's impact on the chest (`0x22`).
    FireImpact,
    /// Fire Blast's impact on the chest (`0x22`).
    FireBlastImpact,
    /// Frostbolt's missile.
    FrostboltMissile,
    /// The frost in a hand, winding up and releasing (`0x15` / `0x16`).
    IceHand(u16),
    /// Frostbolt's impact on the chest (`0x22`).
    IceImpact,
    /// The mark a Frostbolt's slow leaves on the head (`0x14`).
    IceMark,
    /// Frost Armor on the head (`0x14`).
    FrostArmor,
    /// Ruklar's Net (12024, visual 683) in flight and wrapped round the one it holds.
    NetMissile,
    NetState,
    /// Mother Fang's Web (12023, visual 684) in flight and on the one it holds.
    WebMissile,
    WebState,
    /// A Forest Spider's spit: Venom Spit's (16552, visual 4199) goo bolt.
    VenomMissile,
    /// A boar's Rushing Charge (6268, visual 2416): the sprint's burst at its feet (`0x13`).
    SprintCast,
    /// A boss's Frenzy: Enrage's (8599, visual kit 81) red rage on the head (`0x14`), a flash as the power-up begins.
    Enrage,
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
            Kind::FireballMissile => "Spells\\Fireball_Missile_Low.mdx",
            Kind::FirePrecastHand(_) => "Spells\\Fire_Precast_Hand.mdx",
            Kind::FireHand(_) => "Spells\\Fire_Cast_Hand.mdx",
            Kind::FireImpact => "Spells\\MoltenBlast_Impact_Chest.mdx",
            Kind::FireBlastImpact => "Spells\\Fire_ImpactDD_Chest.mdx",
            Kind::FrostboltMissile => "Spells\\Frostbolt.mdx",
            Kind::IceHand(_) => "Spells\\Ice_Precast_Low_Hand.mdx",
            Kind::IceImpact => "Spells\\Ice_ImpactDD_Med_Chest.mdx",
            Kind::IceMark => "Spells\\Ice_Precast_Uber_Head.mdx",
            Kind::FrostArmor => "Spells\\FrostArmor_Low_Head.mdx",
            Kind::NetMissile => "Spells\\Net_Missile.mdx",
            Kind::NetState => "Spells\\Net_State.mdx",
            Kind::WebMissile => "Spells\\Web_Missile.mdx",
            Kind::VenomMissile => "Spells\\GooBolt_Missile_Low.mdx",
            Kind::WebState => "Spells\\Web_State.mdx",
            Kind::SprintCast => "Spells\\Sprint_Cast_Base.mdx",
            Kind::Enrage => "Spells\\BeastRageCaster.mdx",
        }
    }

    /// The attachment id the spell's visual kit hangs it on.
    pub fn attach(self) -> Option<u16> {
        match self {
            Kind::ChargeTrail | Kind::ClapImpact => Some(0x22),
            Kind::ChargeDust | Kind::Slow | Kind::Shout => Some(0x13),
            Kind::ClapHand(id)
            | Kind::FirePrecastHand(id)
            | Kind::FireHand(id)
            | Kind::IceHand(id) => Some(id),
            Kind::ClapBase
            | Kind::FireballMissile
            | Kind::FrostboltMissile
            | Kind::NetMissile
            | Kind::NetState
            | Kind::WebMissile
            | Kind::WebState
            | Kind::VenomMissile => None,
            Kind::SprintCast => Some(0x13),
            Kind::Enrage => Some(0x14),
            Kind::FireImpact | Kind::FireBlastImpact | Kind::IceImpact => Some(0x22),
            Kind::IceMark | Kind::FrostArmor => Some(0x14),
        }
    }

    /// A missile's model plays for as long as the missile flies (it goes with it); every other model is
    /// a flash that removes itself when its first sequence ends.
    pub fn loops(self) -> bool {
        matches!(
            self,
            Kind::FireballMissile
                | Kind::FrostboltMissile
                | Kind::NetMissile
                | Kind::NetState
                | Kind::WebMissile
                | Kind::WebState
                | Kind::VenomMissile
        )
    }

    const ALL: [Kind; 28] = [
        Kind::Enrage,
        Kind::VenomMissile,
        Kind::NetMissile,
        Kind::NetState,
        Kind::WebMissile,
        Kind::WebState,
        Kind::SprintCast,
        Kind::FireballMissile,
        Kind::FirePrecastHand(0x15),
        Kind::FirePrecastHand(0x16),
        Kind::FireHand(0x15),
        Kind::FireHand(0x16),
        Kind::FireImpact,
        Kind::FireBlastImpact,
        Kind::FrostboltMissile,
        Kind::IceHand(0x15),
        Kind::IceHand(0x16),
        Kind::IceImpact,
        Kind::IceMark,
        Kind::FrostArmor,
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
            Kind::FireballMissile => "WoW fireball",
            Kind::FirePrecastHand(_) => "WoW fire precast hand",
            Kind::FireHand(_) => "WoW fire cast hand",
            Kind::FireImpact => "WoW fireball impact",
            Kind::FireBlastImpact => "WoW fire blast impact",
            Kind::FrostboltMissile => "WoW frostbolt",
            Kind::IceHand(_) => "WoW frost hand",
            Kind::IceImpact => "WoW frostbolt impact",
            Kind::IceMark => "WoW frost mark",
            Kind::FrostArmor => "WoW frost armor",
            Kind::NetMissile => "WoW net",
            Kind::NetState => "WoW net hold",
            Kind::WebMissile => "WoW web",
            Kind::VenomMissile => "WoW venom spit",
            Kind::WebState => "WoW web hold",
            Kind::SprintCast => "WoW rushing charge",
            Kind::Enrage => "WoW enrage",
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
        !kind.loops(),
    ))
}

/// Plays sound kit `kit` at the player, not positioned.
pub fn sound(audio: Option<&mut Audio>, kit: u32, what: &str) {
    sound_leveled(audio, kit, what);
}

/// [`sound`], returning the handle that cuts it short (set its level to 0).
pub fn sound_leveled(
    audio: Option<&mut Audio>,
    kit: u32,
    what: &str,
) -> Option<crate::mixer::Level> {
    audio?.play_kit_leveled(kit, None, SCALE, what)
}

/// An effect that goes after a fixed time: a flash cut short of its model's own length.
#[derive(Component)]
pub struct Expires(pub f32);

/// `left` seconds less `dt`; `None` once it is up.
pub fn expire_step(left: f32, dt: f32) -> Option<f32> {
    let next = left - dt;
    (next > 0.0).then_some(next)
}

fn expire(mut commands: Commands, time: Res<Time>, mut fx: Query<(Entity, &mut Expires)>) {
    for (e, mut x) in &mut fx {
        match expire_step(x.0, time.delta_secs()) {
            Some(left) => x.0 = left,
            None => commands.entity(e).try_despawn(),
        }
    }
}

/// [`spawn`], gone after `secs` at the latest.
pub fn spawn_for(
    commands: &mut Commands,
    fx: &Fx,
    kind: Kind,
    owner: Entity,
    secs: f32,
) -> Option<Entity> {
    let e = spawn(commands, fx, kind, owner)?;
    commands.entity(e).insert(Expires(secs));
    Some(e)
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Fx>()
        .add_systems(OnEnter(Phase::Playing), load)
        .add_systems(Update, expire);
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
        for kit in [
            SOUND_CHARGE,
            SOUND_CLAP,
            SOUND_CLAP_HIT,
            SOUND_SHOUT,
            SOUND_FIRE_PRECAST,
            SOUND_FIRE_CAST,
            SOUND_FIRE_IMPACT,
            SOUND_ICE_PRECAST,
            SOUND_ICE_CAST,
            SOUND_ICE_IMPACT,
            SOUND_FROST_ARMOR,
        ] {
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
    fn the_mage_models_ride_the_attachments_their_visual_kits_name() {
        assert_eq!(Kind::FireHand(0x15).attach(), Some(0x15));
        assert_eq!(Kind::FirePrecastHand(0x16).attach(), Some(0x16));
        assert_eq!(Kind::IceHand(0x16).attach(), Some(0x16));
        assert_eq!(Kind::FireImpact.attach(), Some(0x22));
        assert_eq!(Kind::FireBlastImpact.attach(), Some(0x22));
        assert_eq!(Kind::IceImpact.attach(), Some(0x22));
        assert_eq!(Kind::IceMark.attach(), Some(0x14));
        assert_eq!(Kind::FrostArmor.attach(), Some(0x14));
        assert_eq!(
            Kind::FireballMissile.attach(),
            None,
            "a missile's model is its own body"
        );
        assert_eq!(Kind::FrostboltMissile.attach(), None);
    }

    #[test]
    fn an_effect_with_a_lifespan_goes_when_it_is_up() {
        assert!((expire_step(0.5, 0.2).unwrap() - 0.3).abs() < 1.0e-6);
        assert_eq!(expire_step(0.2, 0.2), None, "up exactly");
        assert_eq!(expire_step(0.1, 0.2), None);
    }

    #[test]
    fn a_missile_model_loops_for_as_long_as_it_flies_and_the_rest_are_flashes() {
        // A one-shot's drawn parts go when its first sequence ends (a second for the fireball): a ball
        // that flew longer than that would be a trail of sparks with nothing at its head. A net or a web on the
        // player holds for as long as the hold lasts.
        let held = [
            Kind::FireballMissile,
            Kind::FrostboltMissile,
            Kind::NetMissile,
            Kind::NetState,
            Kind::WebMissile,
            Kind::WebState,
            Kind::VenomMissile,
        ];
        assert!(held.iter().all(|k| k.loops()));
        for kind in Kind::ALL {
            if !held.contains(&kind) {
                assert!(!kind.loops(), "{kind:?} plays once");
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
