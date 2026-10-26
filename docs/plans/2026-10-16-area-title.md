# First-visit area title implementation plan

**Goal:** Match the supplied Elden Ring discovery title for Elwynn Forest after Northshire's gates, Goldshire and Stormwind, once per character.

**Architecture:** A passive native Bevy overlay reads the existing player area authority, resolves registered subareas and parent zones, debounces boundary changes and persists discoveries in the current character folder. The reference and user's explicit instruction establish the design: centered ivory serif text, a soft dark veil, restrained rules and a roughly five-second fade/hold/fade. No input lock or animation/combat changes.

**Tech stack:** Rust, Bevy 0.18.1 UI, installed AreaTable/terrain identity; embedded EB Garamond under OFL 1.1.

1. Verify font provenance/license; embed the unmodified font with OFL and source hash in `crates/wow-souls/assets/fonts/eb-garamond/`.
2. Add `hud/area_title.rs` and pure discovery/journal logic in `hud/area_title/discovery.rs`. Resolve Northshire separately from parent Elwynn; preserve Goldshire and Stormwind identity across their interiors. Check gate placement live before deciding whether a small spatial refinement is necessary.
3. Register the overlay in `hud/mod.rs`. Center with a parent flex container, scale with existing 720p UI, fade text/shadow/veil together. Defer discovery behind menus/dialogue, cancel on death; no movement/combat blocking. Avoid repeating saved discoveries, protect unknown/corrupt save files, write atomically.
4. Test area resolution, debounce/re-entry, pause/death and per-character persistence. Capture native 720p/1080p/ultrawide views, actual area boundaries, fade sequence and reload/no-repeat using isolated saves. Read rather than modify the parallel agent's files.
5. Run scoped fmt, workspace Clippy and install-enabled tests with `-j4`; build and record evidence in `docs/log/2026-10-16-area-title.md`. Update handover in place under 8 KiB. No commits (standing user order).
