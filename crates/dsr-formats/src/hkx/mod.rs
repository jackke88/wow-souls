//! Havok 2015 tagfiles: the animation container, skeletons and spline-compressed animations.

use anyhow::{bail, Result};

pub mod animation;
pub mod skeleton;
pub mod spline;
pub mod tagfile;

use tagfile::{Obj, Tagfile};

/// The file's `hkaAnimationContainer`: root → `namedVariants[i].variant`.
pub fn animation_container(tf: &Tagfile) -> Result<Obj<'_>> {
    let root = tf.root()?;
    for v in root.field("namedVariants")?.array()? {
        if let Some(obj) = v.field("variant")?.ptr()? {
            if obj.type_name() == "hkaAnimationContainer" {
                return Ok(obj);
            }
        }
    }
    bail!("no hkaAnimationContainer in the root container")
}
