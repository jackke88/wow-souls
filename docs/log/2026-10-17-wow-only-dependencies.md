# WoW-only runtime and repository source audit — 2026-10-17

Requested: find and remove remaining old DSR connections, keep code in the project,
and require only WoW 1.12 as a game installation. No commit or upload was requested.

## Findings and changes

| Finding | Result |
|---|---|
| `WOWSOULS_DSR_CACHE` could select a binary extraction cache | Retired; the runtime selects the project animation library only. |
| Missing text clips could fall back to `.wscl` | Removed; missing/invalid text is reported without a binary substitute. |
| Extraction manifests checked source-game file size/mtime | Rejected without opening those external source paths. |
| Old UI/vitals switches could load DSR HUD textures and fonts | Source-art loader removed. Retired selectors are ignored; native UI remains active. |
| DSR GPL extractors were production workspace members | Standalone optional developer workspaces, each with its own explicit manifest/lockfile. No DSR package in the production dependency graph. |
| Twelve engine crates came from a sibling research checkout | Exact code copied to `vendor/benilla`, preserving revision, the two existing local graphics fixes, hashes and licences. |
| Active world manifests existed only in ignored `cache/world` | Copied unchanged to `data/world`; the runtime loads the packaged Elwynn manifest. |
| Quest voices defaulted to a private machine path | Existing active subset (189 recordings, 18.30 MiB) copied unchanged to `data/voices`, with hashes and accurate upstream/provider/content provenance. |
| Six original graphics grading assets were ignored despite `include_bytes!` | Explicit ignore exceptions; actual files added to the project index. These are original engine assets, not Blizzard install files. |
| 92 existing required source/asset/animation files and an AI test table were still untracked | Added to the index without changing their content, including 42 newer Hogger clips. All 1,203 current animation files are now indexed. |
| Several gameplay tests skipped or enumerated old cache folders | Converted to mandatory project-text checks and synthetic text fixtures. |
| Website still required both games and a first-start extractor | Local website/install guide corrected; current DSR-derived content is disclosed. Checklist storage version bumped. |
| Four UI loaders read Georgia from `C:/Windows/Fonts` | They now use the already-bundled OFL EB Garamond font. |
| WoW's own item table names an icon absent from the 1.12 archives | Item-icon preload checks the actual archive chain and uses WoW's shipped question mark when a referenced icon is missing. Added an installed-WoW regression check. |

The production workspace has 3 members and 15 local packages including the engine closure;
all local manifests resolve inside this repository. Standard third-party Rust libraries
remain pinned registry dependencies. The vendor code remains attributed third-party code.

## Preservation and limits

Existing staged animations and all pre-existing code changes were preserved. Game installs,
the external engine checkout and legacy caches were not modified. The new engine/data files
were added to the Git index; no commit was made. Historical docs and attribution are retained.

Animation curves, reference rigs, timing and imported balance values were deliberately
preserved under the prior decision to keep DSR movement. They remain source-derived
`SOURCE_TESTED` content. This work removes install/build/runtime connections; it does not
claim independently authored replacement animations or status `SOURCE_REMOVED`.
ER candidates remain inactive reference material. See `SOURCE-TO-ORIGINAL.md`.

Spoken Player upstream's MIT grant applies to its code, not generated recordings or Blizzard
quest text. `data/voices/README.md` records the actual upstream notice rather than calling
the audio MIT-licensed.

## Verification

- Production tests with WoW configured and intentionally absent DSR/cache paths: **1,263 passed**
  (1,011 game, 241 world library, 4 world CLI, 7 clip-format tests; no ignored tests).
- The mandatory project library checks load all 19 weapon families, locomotion, creature kits,
  reactions, defense, interactions and Mage casts. Every inventory clip is parsed.
- Dependency audit: **15 local packages inside the repository; no DSR production dependency**.
- Local static website validation: **4 pages, no problems**; modified JavaScript syntax checks pass.
- Final post-restoration and font/icon-fix gates: scoped formatting, **1,263 tests**, strict workspace/all-target Clippy and **player build without default/dev features all pass**.
- **Three isolated 20-second runtime checks pass:** combat, retired DSR/UI selectors, and the packaged world. Every run loads all 19 weapon families, 66 gaits, reactions, native UI and bundled voices, reaches resident ground and reports `WOWSOULS_CHECK ok`. The world case confirms the packaged Elwynn path. **No asset-load errors remain.**
- Engine snapshot hashes, all 189 voice hashes and both world-manifest copies were verified. Required source/assets are present in the Git index.
- Logs: `cache/wow-only-audit/tests-complete.log`, `clippy-complete.log`, `fmt-final.log`, `player-build-complete.log`, `runtime-complete.log`; persistent runtime stdout/stderr in `verified-runtime/`; binary/package summary in `verified.json`.
- Verified player executable SHA-256: `3406aabae89afc2a31dfe36bf0b03c5a41786c7a60ccb3d6bb7b77a801d7b8d7`.
- Isolated package and saves retained at `C:\Users\jacob\AppData\Local\Temp\wow-only-6acba1becb6b49848645f320ef5465a3`.

Reproduce with the commands in README and `tools/Test-WowOnly.ps1`. The runtime harness creates
a fresh temporary package containing only the executable and tracked project animation/data
files, launches from a separate empty working directory, uses isolated saves, deliberately
invalid DSR/ER/cache paths and checks default combat, retired selectors, and the packaged world.

During the audit, an automated comment edit incorrectly replaced characters in seven source
files. Those files were recovered from the pre-task snapshot and only the intended changes
reapplied. A snapshot diff confirms the bounded final changes; final checks are rerun afterward.
