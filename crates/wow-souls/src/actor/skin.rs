//! A character body's skin atlas: the 256² composite of the CharSections base skin, face,
//! facial hair, scalp and underwear regions, built by the engine's own plan
//! ([`benilla_formats::CharSections::composite_plan`]). benilla composites off-thread and caches
//! per look for a crowd of players (`entities/skin_composite.rs`, benilla @ 2e82d34); one body
//! built once under the loading cover composites inline here.

use std::sync::Mutex;

use anyhow::Context;
use bevy::prelude::*;

use benilla_assets::LockRecover;
use benilla_formats::{blp_bytes_to_mip_chain, BlpMipChain, Chain, CharSections, ItemDisplay};

use super::CharLook;

/// The composited body atlas for `look`, uploaded; `None` when the tables have no base skin for it.
/// `worn` are the displays whose region textures are painted on (shirt, chest, belt, pants, boots,
/// wrist, gloves, tabard, the engine's bodyslot order); all `None` is the naked body.
pub fn body_atlas(
    look: &CharLook,
    sections: &CharSections,
    chain: &Mutex<Chain>,
    images: &mut Assets<Image>,
    worn: [Option<&ItemDisplay>; 8],
) -> Option<Handle<Image>> {
    // No guild emblem, no tabard preview.
    let plan = sections.composite_plan(
        look.race,
        look.sex,
        look.skin,
        look.face,
        look.facial_hair,
        look.hair_style,
        look.hair_color,
        worn,
        None,
        false,
    )?;
    let atlas = plan
        .run(|path| read_texture(chain, path))
        .inspect_err(|e| warn!("body skin composite failed: {e:#}"))
        .ok()?;
    Some(images.add(benilla_assets::repeat_texture_authored(
        benilla_assets::for_upload(atlas),
        (true, true),
    )))
}

/// Read and decode one BLP off the patch chain.
fn read_texture(chain: &Mutex<Chain>, path: &str) -> anyhow::Result<BlpMipChain> {
    let name = path.replace('/', "\\");
    let bytes = chain
        .lock_recover()
        .read_file(&name)
        .with_context(|| format!("reading texture '{name}'"))?;
    blp_bytes_to_mip_chain(&bytes).with_context(|| format!("decoding texture '{name}'"))
}
