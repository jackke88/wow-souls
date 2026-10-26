# Vendored benilla engine

The 12 engine crates needed by WoW Souls are included here. Building the game no longer
requires a sibling checkout or a developer's research folder. Normal crates.io libraries
remain Cargo dependencies pinned by the root Cargo.lock.

- Upstream: https://github.com/pkuzic/benilla-everwood_graphics
- Revision: `f545081e9f60668b4f5b1252b67b8058c5995bfd`
- Base project: https://github.com/samwhosung/benilla
- Licence: MIT OR Apache-2.0; both texts and THIRD-PARTY.md are retained.
- Snapshot: 2026-10-17, including the working tree's existing changes to
  `benilla-assets/src/shaders/wind_hook.wgsl` and `benilla-world/src/clutter.rs`.
  `local-fixes.patch` preserves those changes relative to the revision.
- `SHA256SUMS` records the copied crate files before any future local development.
  `.gitattributes` preserves those snapshot bytes across line-ending settings/checkouts.
- The small Cargo workspace manifest preserves upstream metadata and dependency features;
  client/network/Lua/Kira packages and their patches are not included.

This remains attributed third-party engine code. Including it in the repository does not
make its authorship ours. No WoW or DSR installation files were copied here.
