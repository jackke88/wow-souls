# First-visit area titles

User reference: Elden Ring's Chapel of Anticipation discovery title. Requested Elwynn Forest immediately outside Northshire's gates, Goldshire and Stormwind, once per character, with a close freely usable font.

## Behavior and appearance

- Native Bevy overlay in `hud/area_title.rs`, pure discovery/journal code in `hud/area_title/discovery.rs`, registered by `hud/mod.rs`.
- Centered ivory mixed-case EB Garamond Regular, 62 logical pixels at the shared 720p UI scale. Parent flex layout centers the lettering; a soft dark veil and two fine, fading rules maintain contrast on bright grass and dark interiors. All layers, including text shadow, share the same opacity. No image generation or extracted Elden Ring UI art.
- Smooth 0.9 s fade in, readable hold until 3.7 s, 1.5 s fade out: 5.2 s total. Player input remains available. Menus, map, inventory and dialogue hide/pause the card and defer a new discovery. Death cancels playback.
- Uses the existing player area authority (terrain plus WMO interiors), not camera position. A candidate must remain stable for 0.55 s. Northshire Valley, Abbey, Echo Ridge Mine and Vineyards (9/24/34/59) do not prematurely discover their parent Elwynn. Goldshire (87) stays distinct; Stormwind's districts resolve to city 1519. Other main zones use their installed parent/name. Additional subzone titles can be added in `destination`.
- Live Northshire crossing: area 9 becomes 12 near WoW `(-9066.7,-43)`, directly beyond the gate. Live Stormwind approach: area 12 becomes district 1617 near `(-9066.5,433.1)`, resolving to Stormwind City (1519). Both announce once, just after the real boundary; no additional coordinate trigger is necessary.

## First-visit storage

`areas-v1.txt` lives in the active character's own save folder. The strict, versioned journal keeps sorted discovered area IDs and uses a temporary file + rename. Re-entry/relaunch does not repeat saved discoveries; another character has a separate journal. Unknown/corrupt formats are preserved, with discoveries remembered for the current session. Normal saves, installed game files and the original overhead !/? were not modified by the checks.

Unattended runs default to a process-specific journal under `cache/area-title/`. Overrides in README: `WOWSOULS_AREA_TITLE=0`, `WOWSOULS_AREAS_SAVE=<path>`, `WOWSOULS_AREA_TRACE=1`.

## Font and cost

Unmodified EB Garamond Regular, downloaded from the official Google Fonts repository at revision `f8c1d3d6cc75e30d77130bdcbfbff27e3b6233fe`. Files: `crates/wow-souls/assets/fonts/eb-garamond/{EBGaramond.ttf,OFL.txt,SOURCE.txt}`. SOURCE records the exact URL and SHA-256. The bundled font is approximately 831 KiB; the procedural veil uses 128 KiB. The font works offline and does not require a Windows font installation.

Copyright 2017 The EB Garamond Project Authors. SIL Open Font License 1.1 permits bundling/embedding with software, including commercial software, with its copyright and license retained. Include OFL.txt in distributions. This is a close visual alternative; it is not claimed to be Elden Ring's original font.

Primary license: https://github.com/google/fonts/blob/f8c1d3d6cc75e30d77130bdcbfbff27e3b6233fe/ofl/ebgaramond/OFL.txt

No image generation or paid API was used. Runtime adds no network, dependency or DSR HUD-art requirement. Exact session token billing is not exposed by the available tools.

## Verification

All commands use `-j4` where applicable. Final logs are under `cache/area-title/`:

- `cargo fmt -p wow-souls -p wow-world -p wscl -p dsr-cache -p dsr-formats -- --check`: clean (`fmt-final.log`). One diagnostic println in `dsr-cache/src/probe.rs` received formatting only so the shared workspace gate passes.
- `cargo clippy -j4 --offline --workspace --all-targets -- -D warnings`: clean (`clippy.log`).
- With both `DSR_DIR` and `WOW_DATA` set, `cargo test -j4 --offline --workspace`: **1,314 passed, 0 failed** (`tests.log`): dsr-cache 90, dsr-formats 52, game 924, wow-world 240 + 4, wscl 4.
- Seven area-title tests cover installed canonical names and actual Stormwind district rows, Northshire exclusions, boundary jitter, pause/death, repeat suppression/stale pending areas, opacity endpoints/hold, journal roundtrip and character isolation, and preserving invalid/future saves. The initial test incorrectly assumed the district was named "Trade District"; it now checks the installed district rows rather than assuming that spelling.
- Main `target/debug/wow-souls.exe` includes the feature. Independent snapshots `cache/area-title/area-preview.exe` and `area-final.exe` were used for unattended checks; the final map and real Stormwind-entry checks used the latter.

Capture harness: `cache/area-title/capture.ps1`. It redirects LOCALAPPDATA and character/quest/item/map/area saves to isolated cache paths, uses a quiet NPC fixture, opens the menu until a fixed time to avoid cold-load timing, then takes a sequence. It verifies the expected discovery count/ID, protects the unknown-format journal and checks `WOWSOULS_CHECK ok`.

**10 live cases passed**, with **37 unique case captures** in `docs/evidence/area-title/`:

| Case | Evidence / result |
|---|---|
| Goldshire 1080p | `goldshire-1080-{25,25.9,26.5,28,29.8,31.5}.png`: hidden/start, fade, hold, fade out, clean disappearance |
| Goldshire 720p | `goldshire-720.png`: same proportions and centering |
| Ultrawide 2560x1080 | `ultrawide.png`: centered, no stretching |
| Stormwind interior | `stormwind-{27,28,31.5}.png`: correct city title, readable against dark wood |
| Northshire exit walk | `gate-{26,28,30,31,32,33,34,36,38}.png`: quiet inside; Elwynn appears after crossing |
| Fresh Northshire | `northshire.png`: no premature Elwynn title/discovery |
| Saved revisit | `revisit.png`: zero discoveries, no title |
| Unknown future save | `unknown-save.png`: title works in-session; original bytes preserved |
| Deferred behind map | `map-defer-{27,31,32,35.5}.png`: map covers the first visit; title starts about 0.6 s after close |
| Real Stormwind approach | `stormwind-entry-{27,32,35,38,40,42,44,46,48,51}.png`: district 1617 resolves to city 1519 and announces once |

Representative full-opacity, fade and finished frames, both real walks, the dark interior and all three resolutions were visually inspected. No remaining area-title implementation work. No commits made.

## Related UI work

`docs/log/2026-10-16-character-roster.md`, `docs/log/2026-10-16-world-map.md`, `docs/log/2026-10-16-compass.md`.

## Disk-space interruption

User paused this work to ask about `C:\Spel\research`. The active `benilla-everwood` engine and `spoken\x` quest voices must remain. Two unused reproducible caches were verified: `research\benilla\target` (10.89 GiB) and `research\gfx-trial\wow-souls\target` (11.49 GiB). Their absolute paths were validated; no links or matching build/game processes were found. Automatic execution policy blocked deletion (`blocked by policy`), including after renewed user authorization. **Nothing was deleted by this chat.** Do not use another shell/API to bypass that rejection. User then requested that UI work resume.
