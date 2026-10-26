# All runtime animations as project text

User instruction (2026-10-16): apply the existing exact DSR-to-editable-text workflow to every
remaining imported animation and keep the actual files in the project, eligible for Git rather
than ignored. This explicitly replaces the earlier decision to ignore `anim/source/`.

Keep existing editable player files intact. Export all current player reactions, defense,
interactions, creature kits and their DS reference skeletons to namespaced `.wsanim` files. Include
the active weapon-family manifests and animation/gameplay tables in the project; exclude obsolete
weapon extraction snapshots. Preserve poses, scale, root motion, event windows and all timings.

The normal runtime reads the project bundle and never checks the DSR install or starts an extractor
for these animations. Explicit legacy-cache overrides remain available for extraction/comparison.
Malformed project clips report an error rather than silently substituting a binary cache. Keep
source provenance: editable/imported DSR data is still source-derived, not an original replacement.
WoW-native M2 animation remains part of the separately required WoW data, outside this DSR migration.

Acceptance: enumerate every active imported clip; all have editable project data or an existing
editable player override, validate numerical round trips, ensure Git ignores none of the animation
bundle, run full loaders with source files inaccessible and an empty binary-cache directory, then
verify unchanged representative animation sequences in the running game and pass required gates.
