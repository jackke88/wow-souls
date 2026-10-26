# Wolf motion experiment

**Goal:** try the user's proposed workflow on the wolves: use the existing DSR numbers as a starting point, expose meaningful motion controls in code, preserve readable Souls combat, and record effort and measured token usage.

**Architecture:** an opt-in, in-memory remix of the loaded `wolf` and `direwolf` attack clips. A new `wolf_motion.rs` owns two small recipes (bite and lunge): phase timing and smooth local rotation curves for neck, head, jaw and tail. The same time mapping resamples body pose, root motion, hit windows and action flags. Existing cache files and the original DS reference skeleton stay intact. Damage, reach bands, bone translations/scales and other species are unchanged. `WOWSOULS_WOLF_MOTION=1` enables it; absent/0 preserves current behavior.

**Tech Stack:** Rust, Bevy/glam, the existing `wscl::Clip`, built-in deterministic video recorder, ffmpeg.

## Design choices

1. **Recommended for this trial:** retain the sampled body/feet, change the rhythm and author a new head/jaw/tail performance. Small scope, preserves the retarget's foot contacts, gives an immediately comparable result.
2. Sparse keyposes for the whole body: more freedom, but reconstructing planted paws requires a contact/IK pass. A useful next experiment if this one is promising.
3. Full procedural quadruped: strongest authoring independence, substantially larger scope than this trial.

The user's "Testa detta med wolves" authorizes implementing and evaluating the editable-motion approach discussed in chat. This is a DSR-derived experiment, **not** a claim of independently authored animation or ownership of the source. It does not remove the whole game's DSR requirement. No commits requested.

## Implementation and acceptance

1. Inspect actual `wolf`/`direwolf` clips and rig layout; preserve current work in this shared, dirty checkout. Record baseline usage from this chat's local session log.
2. Add the pure motion recipes, monotone phase mapping and resampling. Keep first/last frames and total root displacement; change head/jaw/tail rotations with curves that return to zero at both ends. Do not stretch the rig. Resolve the rig by kit and expected topology.
3. Apply only in `monster::load`, after parsing and before attack specs are consumed. Recompute attack length/all hit windows from the resulting clip. Explain source and controls in code and README.
4. Test event/root/pose synchronization, unit quaternions, exact endpoints, stable bone lengths, unsupported kits and real installed cache data. Run scoped formatting, workspace clippy and workspace tests with both install variables. Build with `-j 4`.
5. Record original/remix bite and lunge, plus front and game-camera fight footage. Inspect a dense frame sequence for anticipation, snap, recoil, jaw collision alignment, feet and silhouette. Adjust if needed. Save local comparison video and reproducible capture commands.
6. Write a short Swedish report under `docs/log/`: source, exact changes/measurements, observed quality/limits, wall time, actual token usage delta (including cached input separately), and what a full replacement would still require. Update HANDOVER briefly, without rewriting shared sections.
