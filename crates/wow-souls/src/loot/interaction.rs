//! Optional, verified source interactions and their successful-action markers.
use bevy::prelude::*;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Ground,
    Open,
    Take,
}
impl Kind {
    pub const ALL: [Self; 3] = [Self::Ground, Self::Open, Self::Take];
    pub fn name(self) -> &'static str {
        match self {
            Self::Ground => "ground",
            Self::Open => "open",
            Self::Take => "take",
        }
    }
    pub fn source(self) -> &'static str {
        match self {
            Self::Ground => "a00_7520",
            Self::Open => "a00_7521",
            Self::Take => "a00_7522",
        }
    }
}
pub struct Clip {
    pub kind: Kind,
    pub wow: wscl::Clip,
    pub ds: wscl::Clip,
    pub commit: f32,
}
#[derive(Resource)]
pub struct Kit {
    pub clips: Vec<Clip>,
}
impl Kit {
    pub fn clip(&self, kind: Kind) -> &Clip {
        self.clips
            .iter()
            .find(|c| c.kind == kind)
            .expect("complete interaction kit")
    }
    pub fn load(dir: &Path) -> Result<Self, String> {
        let text =
            std::fs::read_to_string(dir.join("interaction.txt")).map_err(|e| e.to_string())?;
        if text.lines().next() != Some("interaction-version 1") {
            return Err("wrong interaction cache version".into());
        }
        // Reuse strict source metadata checks without binding interaction publication to a full
        // combat re-extraction. The common cache is required by the avatar pose pipeline too.
        crate::dsclip::checked_manifest(dir)?;
        let mut clips = Vec::new();
        for kind in Kind::ALL {
            let name = text
                .lines()
                .find_map(|l| {
                    let w: Vec<_> = l.split_whitespace().collect();
                    match w[..] {
                        ["interact", role, name] if role == kind.name() => Some(name),
                        _ => None,
                    }
                })
                .ok_or("incomplete interaction kit")?;
            if name != kind.source() {
                return Err(format!("wrong clip for {}: {name}", kind.name()));
            }
            let wow = crate::dsclip::read_clip(dir, "human_male", name)?;
            let ds = crate::dsclip::read_clip(dir, "c0000", name)?;
            let commit = wow
                .windows(87)
                .next()
                .map(|w| w.0)
                .ok_or("interaction lacks success marker 87")?;
            if commit < 0.0
                || commit >= wow.duration()
                || wow.windows(wscl::FLAG_INVINCIBLE).next().is_some()
            {
                return Err("invalid interaction timing".into());
            }
            info!(
                "loot clip {} {} duration {:.3} commit {:.3}",
                kind.name(),
                name,
                wow.duration(),
                commit
            );
            clips.push(Clip {
                kind,
                wow,
                ds,
                commit,
            });
        }
        Ok(Self { clips })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Playing {
    pub target: Entity,
    pub kind: Kind,
    /// Negative during motor-driven alignment; nonnegative during playback.
    pub t: f32,
    pub align_age: f32,
    pub committed: bool,
}
#[derive(Resource, Default)]
pub struct Active(pub Option<Playing>);
impl Active {
    pub fn playing(&self) -> Option<(Kind, f32)> {
        self.0.filter(|p| p.t >= 0.0).map(|p| (p.kind, p.t))
    }
}
/// Lid starts lifting after both hands reach the front edge, and rests open at the source
/// success marker. Native Open is 0.666 s; these measured player times preserve that duration.
pub const LID_START: f32 = 0.5333333;
pub const LID_END: f32 = 1.2;
pub fn lid_fraction(t: f32) -> f32 {
    ((t - LID_START) / (LID_END - LID_START)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lid_waits_for_contact_and_finishes_at_open_success() {
        assert_eq!(lid_fraction(0.3), 0.0);
        assert_eq!(lid_fraction(1.2), 1.0);
        assert!((lid_fraction((LID_START + LID_END) * 0.5) - 0.5).abs() < 1e-5);
    }
}
