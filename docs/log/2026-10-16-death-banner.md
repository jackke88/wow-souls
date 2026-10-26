# Single death banner

The user's screenshot showed two overlapping `YOU DIED` texts at different sizes.

## Cause and change

`hud::banner` supplies either the native text/band/fade or the legacy DSR image banner.
The native UI deliberately skips loading `HudArt`. `hurt::plugin` treated that missing
resource as a reason to create its old 84 px fallback, so the native 62 px text and
the fallback appeared together on every death.

The fallback now runs only when **both** the native theme and legacy art are absent.
The banner's layout, timing, colors and respawn behavior are unchanged.

## Verification

- The real `OnEnter(Playing)` systems are registered together in a regression test,
  without legacy art. Before the fix the test found two `YOU DIED` texts; afterward
  it found one. All seven banner tests pass.
- Live baseline: the unmodified executable reproduced the exact overlapping text.
- Live after the fix: native UI, legacy DSR HUD, `WOWSOULS_HUD=0`, and legacy UI
  with missing HUD art all show one message. Five frames per run cover death +0.1,
  +1.5, +3, +4.4 and +5.5 seconds. Native/legacy reach black before respawn; the
  message clears on respawn in every mode. All five baseline/after runs exit zero
  with `WOWSOULS_CHECK ok`.
- Workspace tests with installed source data enabled: **1,362 passed**, including
  the new regression (964 game tests). The isolated regression also passes in
  separate legacy/plain processes.
- Scoped workspace format check, strict all-target workspace Clippy, production
  build and diff whitespace check pass. Format also reflowed one assertion in a
  concurrent `dsr-cache::monster` test; no logic changed there.
- The missing empty-flask icon warning occurs before and after this change. The
  missing-cache run intentionally falls back to the older procedural combat kit.

Local test harness, executables, screenshots and logs: `cache/death-banner/`.
Runs use isolated saves and the read-only WoW data. DSR/ER install paths point at
nonexistent directories. The harness explicitly selects the old cache to exercise
the legacy HUD comparison; the missing-art case selects a nonexistent cache.

Review captures and gate logs are also saved under
`C:/Users/jacob/Documents/GitHub/wow darksouls/docs/evidence/death-banner/`.

No commit.
