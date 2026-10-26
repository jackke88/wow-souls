# Graveyard soul binding — 2026-10-16

Jacob approved explicit binding at a memorial stone, with a tutorial from the same NPC as the inn quest.

## Behavior and ownership

- `soulbind.rs` owns sites, saving and interaction validation. Sites come from non-event Spirit Healer 6491 spawns: Northshire 87047, Goldshire 79628, Eastvale 17650. The healer remains hidden. `soulbind/art.rs` builds the stone, collider and soul rune.
- Interact/Talk sends `BindRequest(guid)`. Range 3.5 yd, height 3 yd, grounded/live/free body and no alerted living enemies are required. Persist first, then set active and publish `SoulBound { site }`. Rebinding is free. Damaged/unknown saves are preserved, and a failed write neither changes the active site nor emits success.
- `soulbind-v1.txt`: `wssoulbind 1`, `site <GUID>`. Stored in the character slot, included in legacy file migration; `WOWSOULS_BIND_SAVE` overrides it. Unattended default paths are isolated by PID.
- Starting again spawns beside the saved memorial; `WOWSOULS_AT` explicitly overrides the starting position. Death still selects the actual binding. A character without a binding keeps the existing start behavior.
- `hurt::respawn` keeps the existing death banner black and holds the dead body's position until the destination tile is presentable and a valid ground ray succeeds. Then it restores the existing health/stamina/action/poise and emits the normal `Respawned`. Bloodstain/currency rules are unchanged.

## Quest integration for the parallel quest work

`quests/soul_tutorial.rs` owns a small runtime overlay: **quest 9000101**, title **Bind Your Soul**, Falkhaan Isenstrider **6774** gives and takes it. Falkhaan also gives imported **2158 Rest and Relaxation**, which is untouched. The tutorial uses a negative object objective for authored memorial **9000101**, one real binding, 40 XP, no item or money rewards.

The adapter only credits when `Binding::site()` is valid, including after accepting the quest with a previously saved binding. It saves the ordinary quest log, uses the existing tracker, markers, reward and compass-to-ender flow, and never marks mere proximity as completion. `SoulBound { site }` is available to other consumers. No changes to `wow-world`'s generic quest engine or imported manifests were needed.

Animation work is untouched. No commits requested or made.

## Verification

- Required scoped formatting and `git diff --check`: pass.
- Workspace tests with **both** `DSR_DIR` and `WOW_DATA`: **1,340 pass** (dsr-cache 90, dsr-formats 52, game 949, wow-world 241 + CLI 4, wscl 4). `cache/soulbind/tests-final.log`.
- The final presentation-only prompt change was followed by the complete game test target again: **949 pass** (`tests-ui-final.log`), a fresh game build, and another live bind/death/respawn.
- Strict workspace/all-target Clippy is blocked by a pre-existing `clippy::useless_conversion` at `quests/objects.rs:264` (`dir: dir.into()`, that file was already modified by the parallel quest work). The file was left to that work. All-target Clippy with only that known lint allowed passes, including after the final UI change: `clippy-ui-final.log`. Strict output: `clippy.log`.
- **12 new feature tests**: strict/corrupt/unknown save handling, failed-write rollback, switching/restoring/isolation, DB site selection, range/height, actual ECS binding validation/success emission, tutorial accept/objective/save/turn-in/prebound sync, and actual collision-backed respawn hold/release.

Live harness: `cache/soulbind/live.ps1`. Each process has isolated character, quest, item, souls, world-state and area saves plus an isolated LOCALAPPDATA. Game windows run unattended and never capture input. Every scenario below exits with `WOWSOULS_CHECK ok`:

1. Falkhaan's real dialogue: select *Bind Your Soul*, accept it; saved log is incomplete (0/1).
2. Interact at the actual Northshire stone: saved GUID **87047**, objective complete (1/1), compass points back to Falkhaan. Walk away and die carrying 123: carried becomes 0, 123 remain in a bloodstain, the player respawns beside the stone on ground.
3. Restart without `WOWSOULS_AT`: saved Northshire binding loads and the player starts beside the stone; the bloodstain and tutorial progress persist.
4. Falkhaan's real reward dialogue: hand in **9000101**, saved status rewarded, **40 XP**. Imported inn quest remains independently available.
5. Unloaded destination: explicitly start near Eastvale, retain the Northshire binding, die there. The dead body is held at Northshire while that tile streams for about **3.3 s**, then restores at Bevy `(182.50557, 79.82413, 8948.204)`. Zero carried souls preserve the waiting stain.
6. Final presentation check: the stone is readable in the world's custom renderer (painted texture/material), the rune identifies it, the success text is clear, and the interact prompt waits five seconds while confirmations show. A repeated live recovery also returned the old stain's souls before the next death.

Logs and frames: `cache/soulbind/` and `cache/soulbind/final/`. Shareable PNG copies are in the external workspace's `docs/evidence/soul-binding/`. The runtime overlay and public `SoulBound { site }` are the integration points for subsequent quest work.
