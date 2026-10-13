//! Readers for Dark Souls Remastered's data formats: DCX compression, BND3 archives, DS1 TAE
//! animation events, PARAM files with their PARAMDEFs, Havok 2015 tagfiles (skeletons,
//! spline-compressed animations), TPF texture archives, DRB UI layouts and CCM bitmap fonts.
//!
//! GPL-3.0-or-later. Ported from SoulsFormatsNEXT (DCX, BND3, TAE, TPF, DRB, CCM), SoulsAssetPipeline's
//! Havoc tagfile reader and its spline decompressor (Meowmaritus, Katalash), itself after HavokLib
//! (Lukas Cone). Layouts were checked against the user's own install; see docs/PROVENANCE.md.

pub mod bnd3;
mod bytes;
pub mod ccm;
pub mod dcx;
pub mod drb;
pub mod hkx;
pub mod param;
pub mod tae;
pub mod tpf;
