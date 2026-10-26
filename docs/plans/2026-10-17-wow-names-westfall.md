# WoW names, memorial art and Westfall Implementation Plan

> Execute the requested changes in this checkout, preserving existing work. No commits were requested.

**Goal:** Use readable WoW motion names, replace the memorial marker, and ship the complete non-seasonal Westfall database selection with an animation worklist.

**Architecture:** Keep motion curves and combat windows unchanged; rename the project-facing modules, rigs and actions with a reversible provenance map. Import Westfall using client area ancestry, and include Elwynn and Westfall in one packaged world. Existing custom kits remain; unsupported bodies use the client's native animation fallback and are explicitly listed for custom animation work.

**Tech Stack:** Rust, Bevy, wscl text clips, vMaNGOS TSV, WoW 1.12 MPQ/DBC, generated transparent PNG.

### 1. Preserve and inventory
- Snapshot the affected sources and animation library under ignored `cache/`.
- Record the old-to-new clip/module names; fingerprint motion payloads before renaming.

### 2. Westfall data
- Extend `crates/wow-world/src/valley.rs` and the importer CLI with Westfall and a combined scope; no Elwynn level cap in Westfall.
- Test ancestry, exclusion of adjacent zones, high-level quest inclusion, and complete manifest round trips.
- Import `data/world/westfall.txt` and `data/world/elwynn-westfall.txt` from the existing whole-world TSV export. Switch the shared runtime default.
- Retain all database quest relations, item/loot/vendor dependencies, display probabilities and patrols.

### 3. NPC population and worklist
- Keep existing kits on supported bodies; implement native WoW fallback for the new unsupported creature families so their spawn records are present in-game.
- Generate a categorized report containing every selected Westfall NPC and quest, with entry/quest IDs, spawn counts, models, available kits and missing custom animation groups.
- Test native fallback selection and keep Elwynn classification unchanged.

### 4. Memorial decal
- Copy the generated original asset into `crates/wow-souls/assets/soulbind/`, preserve alpha, and save its prompt/provenance.
- Replace the black inset and neon bars in `soulbind/art.rs` with two correctly oriented decal meshes. Use a restrained pulse.
- Inspect front and back in actual game screenshots.

### 5. Readable motion code
- Rename `dsclip`, `dsloco`, `dsview`, source-specific resource/field names and active raw clip/rig names to gameplay/body/action names.
- Update all playback tables and tests atomically; preserve source attribution in comments and the provenance map.
- Verify every motion payload has the same semantic fingerprint, all indexed clips load, and no active runtime identifier contains the old source prefix.

### 6. Validation and handover
- Run scoped formatting, workspace tests and Clippy with `-j 4`.
- Build and run isolated WoW-only checks, a Westfall population/quest check, and front/back memorial screenshots.
- Update the handover and a dated log with actual counts, categorized remaining animation work and verification evidence.

### 7. Graveyard guidance (user follow-up)
- Show the client's native spinning yellow question mark over the Northshire memorial while Bind Your Soul is accepted and unfinished.
- Guide that objective on the compass at any distance; after binding, use the normal completed-quest return bearing to Falkhaan.
- Discover other memorials within 120 horizontal yards using the embedded rune, site name, distance and bound state, even without a quest.
- Verify acceptance, abandonment, completion, discovery boundaries and camera bearings with tests, then inspect isolated live captures before/after binding and at a Westfall graveyard.
