# Stable weapon rest implementation plan

**Goal:** Remove the two-second hand twitch and carry each weapon in its DSR passive stance outside actions.

**Architecture:** Keep the stance's continuous elapsed time separate from the looping sample phase. Reuse the existing action crossfade, but never restart it on a breathing-loop seam. Standing uses the full source idle; walking retains its upper-body carry pose over the existing locomotion legs, with a short lower-body transition. Add missing ranged idle clips from the installed DSR motion categories.

**Tech stack:** Rust, Bevy, local DSR HKX/TAE/PARAM, wscl cache. Existing user authorization covers the Souls animation direction and this correction. No commits or engine changes.

1. Reproduce the two-second reset: source idle seam is below 0.05 degrees, while runtime `layers` treats modulo time as a new action. Record the current greatsword pose.
2. In `wow-souls/src/dsclip.rs`, add a rest clock and wrap time only after blend weights are computed. Test several complete loops, attack interruption/re-entry, and smooth lower-body transitions.
3. Preserve source carry arms while walking; keep action/interaction/airborne priority. Export bow `a14_0000`, crossbow/gun `a16_0000`, thrown `a00_0000` in `dsr-cache/src/cache.rs`. Validate all 19 idle pairs and their seams.
4. Run installed-cache tests, game tests, scoped fmt and workspace clippy. Rebuild, record normal runtime for at least three loops and attack-to-rest, inspect standing/walking and ranged stills.
5. Update `HANDOVER.md` and `WARRIOR-WEAPONS.md` with evidence and limitations: walking legs are still WoW; gun remains a crossbow adaptation; no automatic sheathing or new proximity combat state is introduced.

Source: https://soulsmodding.com/doku.php?id=ds1-refmat:wepmotioncategory-list
