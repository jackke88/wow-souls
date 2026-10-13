# First loot slice — approved direction

The user approved the proposed slice with "Kör" on 2026-10-09. Build an animated fixed chest,
corpse pickup, original acquisition UI, a minimal inventory and persistence. Keep WoW quests
and base loot; authored treasures are an additional layer. No commits without a user request.

Use DS player clips 7521 (open), 7522 (take from chest), 7520 (ground pickup), extracted through
the existing retarget gates. Play the WoW chest's real Closed/Open/Opened animations. Opening
and collecting are separate contextual actions, so an interrupted pickup leaves an open,
unclaimed chest. X / gamepad Y interacts; E / LB remains guard. Combat can interrupt interaction.
Require a grounded player, short motor-driven alignment and a reachable target before starting.

First fixed treasure: three Abbey Salves, a custom usable item restoring 30 health. V / gamepad
X uses a salve only when alive, idle and hurt. Tab / Start opens the inventory, which supports
keyboard / gamepad navigation and shows recent acquisitions. Inventory does not pause danger;
opening it blocks gameplay input, damage closes it. Acquisition cards do not capture combat input.

One shared inventory type lives in wow-world. The quest engine is not present in this checkout
yet; expose actual-acquisition events and use the canonical inventory for future G3 integration.
Never fabricate quest acceptance: negative-chance quest loot requires an explicit eligibility
set, default empty. Support baseline loot grouping and reference tables with a recursion limit;
roll once per corpse, never per interaction. Keep authored one-time claims separate from base
chests and their respawn rules. First placement is surveyed and verified live before finalizing.

Persist inventory and authored claim state together using a versioned strict save. Write a
candidate save before updating runtime ownership or success UI. Reject corrupt saves visibly
instead of quietly replacing them. Saved open state survives reload; collected treasures never
duplicate. Unattended runs use an explicit isolated save path and leave player progress alone.

The initial approval proposed original charcoal/ivory/bronze UI. The user's later instructions
on 2026-10-09 supersede that visual direction: match the supplied Dark Souls souls-counter panel,
use the installed Dark Souls panel texture, use real WoW item icons (including Broken Fang),
and use WoW's exact blinking loot VFX. Reuse native art as much as possible in this prototype.
Pickup rows show icon, item name and quantity in a compact Souls frame; inventory uses dense
icon rows and a clear selected item. Existing combat HUD stays in place during this slice.
Use anchored layouts, readable text and keyboard/gamepad navigation.

Validation covers duplicate claims, overflow, failed writes, round-trip/reload, quest eligibility,
reference/group loot, interrupted opening/pickup, real retarget metrics, source comparison,
hand/lid contact, health consumption, UI at 1280x720 and a second aspect, and existing gates.

Source survey: ../superpowers/specs/2026-10-09-loot-ui-research.md.
