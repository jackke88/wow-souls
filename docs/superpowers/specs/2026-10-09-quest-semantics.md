# Northshire quest semantics: availability, cards, text, UI, kill and loot rules (input for the quest-engine plan G3)

Status: research report, 2026-10-09. Read-only research; nothing here is built. Author: research agent for the wow-souls project.
Inputs: `cache/world/northshire.txt` (the manifest, 28 quests), `cache/mangos/tsv/*.tsv` (the raw exports), `docs/HANDOVER.md` ("Standing orders", "Quests: data import"),
`docs/superpowers/specs/2026-10-09-northshire-quests-design.md`, and the vMaNGOS source (https://github.com/vmangos/core, branch `development`) read through WebFetch on 2026-10-09.

## 0. How to read this report

Every claim carries a tag:

- **[V:data]** verified in the manifest or the TSV exports (the numbers were recomputed by script; a second script compared the 28 manifest quest records with the TSV rows field by field: 0 mismatches over title, details, MinLevel, QuestLevel, PrevQuestId, NextQuestId, ExclusiveGroup, NextQuestInChain, RewXP, RewOrReqMoney, QuestFlags, SpecialFlags, RequiredClasses, RequiredRaces, patch).
- **[V:src]** verified in the cited vMaNGOS source file and function. Caveat for all of these: WebFetch answers through a small summarising model, so I asked for behaviour with function and variable names and cross-checked key points with a second, differently worded fetch; code blocks that came back verbatim are the basis of the precise statements. I did not have a local clone and did not touch the repack's own core (`C:\Spel\MaNGOS`), so the rules are those of vMaNGOS `development` on 2026-10-09. The quest rules involved are old MaNGOS-Zero code and unlikely to differ in the repack, but the repack's core version is **[N]**.
- **[V:doc]** and **[V:conf]**: verified in a cited web page (AzerothCore wiki, quest_template) or in the vMaNGOS config template `src/mangosd/mangosd.conf.dist.in` (defaults only; the repack's own config was not read). `[V:data tsv]` means verified by scanning the whole-database TSV exports, not only the manifest.
- **[I]** inferred (from general knowledge of the 1.12 client or from reading between the lines); each is marked so the plan can decide whether to rely on it.
- **[N]** not found: the data or the sources I could read do not say.

Quote policy: this report quotes at most one short code line (section 1.5, the decisive guard); everything else is paraphrase plus exact identifiers (file, function, field names). Quest texts are Blizzard's; the cards give lengths, placeholders, a paraphrased gist and the line of the manifest where the verbatim text lives.

## Summary of what the plan must know (the 12 decisive findings)

1. A Human Warrior can ever take **14 of the 28** quests: 783, 5261, 7, 33, 3100, 15, 21, 54, 18, 6, 3903, 3904, 3905, 2158 [V:data + V:src, simulated, section 1.12]. The other 14 are: 5 other class letters (3101-3105), the Priest quest 5623, the Warlock quest 1598, the Stormwind chain 344-347 (givers outside the valley, level 20), and three quests with no giver at all (5805, 7961, 7962).
2. **A negative ExclusiveGroup does NOT make quests exclusive.** `ExclusiveGroup -18` (quests 18 and 33) means "each-from-all": 18 and 33 can both be in the log and both be finished, and any quest that follows a member needs **all** members rewarded. Result: **3903 Milly Osworth needs both 18 and 33**, and (a side effect of the same rule) **6 Bounty on Garrick Padfoot, whose PrevQuestId is 18, needs 33 as well** (section 1.5). Whether Blizzard's 1.12 did that for 6 is [N]; it is what vMaNGOS does with this data.
3. `NextQuestId` only creates a prerequisite link (3903 gets 18 and 33 as prerequisites from it). `NextQuestInChain` is what makes the NPC offer the next quest in the same window on turn-in: 783->7, 5261->33, 15->21, 3903->3904->3905 (section 1.6).
4. **MinLevel 2 blocks 5 quests (18, 6, 3903, 3904, 3905)**, and G3 has no XP (G4 comes later). Without a decision the player stays level 1 and those five can never be taken (section 1.8, open question 1).
5. Quests whose source item is also their required item (54, 346, 3100-3105, 3905) are **complete the moment they are accepted**, and so are the quests with no objectives (783, 5261, 3903, 2158, 344, 345): vMaNGOS completes right after accept when the objectives are already met (HandleQuestgiverAcceptQuestOpcode). Their "?" shows at the ender immediately.
6. **Enders outside the valley:** 54 (Marshal Dughan, Goldshire) and 2158 (Innkeeper Farley, Goldshire) can be accepted but never turned in; 54 is the last quest of the McBride line and carries the only shield any of these quests gives (Pikeman Shield). Proposal in section 2.3.
7. **5805 Welcome!, 7961 and 7962 cannot be offered**: no giver row exists; 7961/7962 are Blizzard test quests (Designer Island, Jon LeCraft) that only landed in the manifest because their ZoneOrSort is 9. 5805 is the Collector's Edition voucher quest (section 2.2).
8. Kill credit is by exact creature entry: Kobold Vermin 6 (x10, quest 7), Kobold Worker 257 (x10, quest 15), Kobold Laborer 80 (x12, quest 21). No object is a kill target; no negative id occurs in the 28 (section 5.1).
9. Quest items drop only while the quest still needs them: `ChanceOrQuestChance` negative = quest drop at abs(value) percent, filtered per player by "quest in log and counter below the requirement" (section 5.2). Red Burlap Bandana 80% (thug 38, 77 spawns), Tough Wolf Meat 80% (Timber Wolf 69 + Young Wolf 299, 73 spawns), Garrick's Head 100% (one spawn).
10. The 8 Milly's Harvest come from **40 crates** (object 161557, chest type 3, loot 10119, consumable, 180 s respawn) in the Vineyards (section 5.3).
11. Placeholders in the 28 quests: only `$B $b $N $n $c $C` and three `$g...;` gender switches (`brother:sister`, `man:woman`, `priest:priestess`) plus one literal line feed; no `$r $R $p`, no `|c` colour codes, no `<name>` forms (section 3).
12. Not exported but used by vMaNGOS rules: `MaxLevel`, `RequiredCondition`, reputation requirements, `RewMailTemplateId`, emotes, `item_template.start_quest`, `gameobject_template.data6..data23`, scripts (`quest_start_scripts`, `quest_end_scripts`), `npc_text`/gossip. Recommended re-export list in Appendix A.

---

## 1. Availability rules as vMaNGOS applies them

### 1.1 What the loader builds (ObjectMgr::LoadQuests, src/game/ObjectMgr.cpp)

Note: in the `development` branch the file is `src/game/ObjectMgr.cpp`, not `src/game/Objects/ObjectMgr.cpp`.

- **Which row of a quest is used (`patch`).** The loader selects, per `entry`, the `quest_template` row with the highest `patch` that is not above the configured WoW patch (a correlated sub-select on `patch`) [V:src]. Our export did the same with 10 as the patch of 1.12 [V:data, `tools/export-mangos.ps1`, function `Latest`]. Patch numbers map to client versions in ascending order (the source has constants such as `WOW_PATCH_110`); that 10 = 1.12 is the project's own reading [I]. In the 28: patch 0 for 25 quests, 1598 patch 7, 7962 patch 4, 7961 patch 9; all three are at or below 10, so all are "live" in 1.12 [V:data]. Older rows of the same quest (other texts or rewards) are not exported [N], and by the loader's rule they do not matter for 1.12.
- **`prevQuests` of a quest** (signed ids): its own `PrevQuestId` is pushed onto its own list (rejected if the target quest is a breadcrumb); and for every quest A with `NextQuestId` N != 0, A's id is pushed onto the list of quest |N|, **negative if N is negative, positive otherwise** [V:src]. So a quest can have prerequisites that its own row does not mention (3903).
- **`prevChainQuests`**: every quest whose `NextQuestInChain` is B puts its id on B's list [V:src]. A missing target resets the field to 0 with an error.
- **Exclusive groups**: `ExclusiveGroup` != 0 inserts (raw signed group, quest id) into one multimap; no special handling for the sign at load time, no check for single-member groups [V:src].
- **Breadcrumbs**: `BreadcrumbForQuestId` links; none of the 28 uses it and no other quest in the whole database points at them [V:data].
- **Class and race masks**: 0 = any; a non-zero mask without any playable bit is reset to 0 with an error; partly valid masks are kept [V:src].
- **Source item**: `SrcItemCount` 0 is raised to 1; a non-existent `SrcItemId` is cleared [V:src].
- **Derived flags**: any `ReqItemId` sets the internal DELIVER flag, any `ReqCreatureOrGOId` sets KILL_OR_CAST, a non-zero `LimitTime` sets TIMED [V:src]. These decide which of the counters (item counts, kill counts) the runtime keeps; a quest with none of them has no objectives.
- **Method**: `Method` 0 = auto-complete, 1 = disabled (quest inactive), 2 = deliver (normal) [V:src QuestDef.h/.cpp]. All 28 have Method 2 [V:data].
- **Money and XP columns** are not validated; `RewXP` is direct XP (section 1.8), `RewOrReqMoney` positive = reward, negative = price [V:src QuestDef.cpp `GetRewOrReqMoney`].

### 1.2 What "can the player take it" checks (Player::CanTakeQuest, src/game/Objects/Player.cpp)

In this order, all must hold [V:src]:

1. `MaxLevel` (when non-zero) is not below the player's level. **MaxLevel was not exported [N]** (very likely 0 for all 28; confirm in the re-export).
2. `SatisfyQuestStatus`: no log entry with a status other than NONE. A finished non-repeatable quest keeps status COMPLETE with `m_rewarded` set, so it can never be taken again.
3. `SatisfyQuestExclusiveGroup` (section 1.5).
4. `SatisfyQuestClass` and `SatisfyQuestRace`: mask 0 passes, otherwise `(mask & player's class/race bit) != 0`.
5. `SatisfyQuestLevel`: player level >= `MinLevel`.
6. `SatisfyQuestSkill`: `RequiredSkill` 0 for all 28 [V:data].
7. `SatisfyQuestCondition`: `RequiredCondition` **[N] not exported**.
8. `SatisfyQuestReputation`: min/max reputation columns **[N] not exported**.
9. `SatisfyQuestPreviousQuest` (section 1.4).
10. `SatisfyQuestTimed`: only one timed quest at a time; no quest here is timed (LimitTime 0) [V:data].
11. `SatisfyQuestNextChain` and `SatisfyQuestPrevChain` (section 1.6).
12. `SatisfyQuestBreadcrumbQuest` and `SatisfyQuestDependentBreadcrumbQuests`: inert here.
13. `Quest::IsActive()` (Method not disabled).

Accepting additionally needs `CanAddQuest`: a free log slot and room for the source item (`SatisfyQuestLog`, `CanGiveQuestSourceItemIfNeed`) [V:src]. The vMaNGOS log holds 20 entries in the 1.12 client [I].

Which quests an NPC shows differs slightly by purpose [V:src]:
- **Quest menu entries** (`Player::PrepareQuestMenu`): giver quests are listed when status is NONE and `CanTakeQuest(.., false)` passes, so a quest above the player's level is **not listed at all**; ender entries are listed for COMPLETE (not rewarded) and INCOMPLETE quests.
- **Overhead marks** (`WorldSession::GetDialogStatus`): uses `CanSeeStartQuest` (checks 3, 4, 6, 7, 8, 9, 11, 12 and 13: everything except MaxLevel, status, level and the timed rule), plus the high-level hide rule: with `Quests.HighLevelHideDiff` (default 7 in `mangosd.conf.dist.in`) the mark is hidden while player level + 7 < MinLevel [V:src + V:conf]. For these 28 the rule never hides anything for a level-1 character (largest MinLevel of a quest the Warrior can take is 2; the level-5 and level-20 quests are not the Warrior's).

### 1.3 Quest state per player

Statuses: NONE, INCOMPLETE, COMPLETE, FAILED, plus the boolean `m_rewarded`. "Current" (used by chain checks) means INCOMPLETE, or COMPLETE and not yet rewarded. A non-repeatable quest ends as COMPLETE + rewarded; a repeatable one returns to NONE; `GetQuestRewardStatus` is always false for repeatable quests [V:src]. No quest of the 28 is repeatable (SpecialFlags 0 for all, section 1.10) [V:data].

### 1.4 Player::SatisfyQuestPreviousQuest (the core of the dependency graph)

Paraphrase of the function [V:src]:

- No entries in `prevQuests` -> true (first quest of a line).
- Otherwise walk `prevQuests`; entries whose quest template or log entry does not exist are skipped. For each entry with id |p|:
  - **p > 0 and that quest is rewarded**: if the previous quest's `ExclusiveGroup` is >= 0 -> **true** immediately (one-from-all). If it is negative (each-from-all) -> true only if **every other quest in that group is also in the log and rewarded**; otherwise the function returns **false at once** (it does not try the remaining entries).
  - **p < 0 and that quest is current (in the log, not rewarded)**: the same split, but with "current" instead of "rewarded".
- If no entry qualified -> false (the "Has only positive prev quests in non-rewarded state" case).

Consequence: **a positive PrevQuestId needs the previous quest rewarded (turned in), not merely completed.** A negative id (never used in these 28) would mean "available while the earlier quest is still in the log".

### 1.5 ExclusiveGroup: positive vs negative (answer to the brief)

`Player::SatisfyQuestExclusiveGroup` starts with the guard

    if (qInfo->GetExclusiveGroup() <= 0)

and returns true there (my reading, not source text: zero and NEGATIVE groups never block taking a quest), so only **positive** groups matter in that function: such a quest can be taken only if no other quest of the group has status INCOMPLETE or COMPLETE (a finished one stays COMPLETE, so one-of-the-group-only for good; quests with no entry, NONE or FAILED do not block) [V:src]. No quest of the 28 has a positive group [V:data].

A **negative** group is used only inside `SatisfyQuestPreviousQuest` (section 1.4): "each-from-all", i.e. all quests of the group are prerequisites together for whatever follows one of them. For `ExclusiveGroup -18` = {18 Brotherhood of Thieves, 33 Wolves Across the Border} [V:data: these are the only two quests in the whole DB with group 18 or -18]:

| Situation (Human Warrior, level 2) | 18 / 33 takeable? | 3903 Milly Osworth | 6 Bounty on Garrick |
|---|---|---|---|
| 783 and 5261 rewarded, nothing else | both takeable, and 33 stays takeable while 18 is in the log (not exclusive) | no | no |
| 18 rewarded only | - | **no** (33 missing) | **no** (33 missing) |
| 33 rewarded only | - | **no** (18 missing) | no (18 missing) |
| 18 **and** 33 rewarded | - | **yes** | **yes** |

(Table produced by running the vMaNGOS rules as a script over the manifest data; outputs: "18 rewarded: 3903 False, 6 False; 33 rewarded: False, False; both: True, True" [V:data + V:src].)

So: 3903's prerequisites are [18, 33] because both 18 and 33 name 3903 in `NextQuestId` (section 1.1); 3903 has `PrevQuestId` 0 in its own row, which hides the dependency if one only reads the quest's own row. Quest 6 has the ordinary-looking `PrevQuestId 18`, but because 18 sits in a negative group the same all-members rule applies to it. **Whether the original 1.12 server did that for quest 6 is [N]**; the plan should state the choice (replicate vMaNGOS exactly as default, per the standing order "exactly as WoW Classic has them" and the data being the source of truth; open question 2).

### 1.6 NextQuestId, NextQuestInChain, breadcrumbs

- **NextQuestId** (18 -> 3903 and 33 -> 3903 are the only non-zero values among the 28): no runtime effect of its own; its only effect is the derived prerequisite on the target (section 1.1) [V:src]. It is positive for both, so it behaves as an ordinary "previous quest rewarded" link; a negative value would mean "available while the source is in the log" [V:src].
- **NextQuestInChain** (783->7, 5261->33, 15->21, 3903->3904, 3904->3905, 343->344->345->347->346): at reward time `HandleQuestgiverChooseRewardOpcode` calls `Player::GetNextQuest(giver, quest)` which returns the chain's next quest **only if the same NPC (or object) has it in its quest relations**, and the quest details window for it opens immediately (the player may still decline) [V:src]. In all chains of the 28 the ender of A is the giver of B, so the auto-offer always fires [V:data: 783 ends at McBride and McBride starts 7; 5261 ends at Eagan who starts 33; 15 ends at McBride who starts 21; 3903 and 3904 end at Milly who starts 3904 and 3905; 344 ends at Paxton who starts 345; 345 ends at Foreman Oslow who starts 347; 347 ends at Paxton who starts 346].
- It also constrains availability: `SatisfyQuestNextChain` refuses a quest whose chain successor is in the log (status INCOMPLETE or COMPLETE), and `SatisfyQuestPrevChain` refuses a quest while a quest that names it as `NextQuestInChain` is still **current** (in the log, not rewarded) [V:src]. In practice: 7 cannot be taken while 783 is still unrewarded, and so on; no deadlock exists in the data (the simulation completes every reachable quest).
- The auto-offer does NOT happen on accept (`HandleQuestgiverAcceptQuestOpcode` sends no next-quest details) [V:src].
- **Breadcrumbs** are not used by any of the 28 [V:data].

### 1.7 RequiredRaces / RequiredClasses

Masks are `1 << (id - 1)`; ids from `SharedDefines.h`: Warrior 1, Paladin 2, Hunter 3, Rogue 4, Priest 5, Shaman 7, Mage 8, Warlock 9, Druid 11; Human 1, Orc 2, Dwarf 3, Night Elf 4, Undead 5, Tauren 6, Gnome 7, Troll 8. `RACEMASK_ALL_PLAYABLE` = 255, `CLASSMASK_ALL_PLAYABLE` = 1503 (0x5DF) [V:src]. Hence class bits: Warrior 1, Paladin 2, Hunter 4, Rogue 8, Priest 16, Shaman 64, Mage 128, Warlock 256, Druid 1024; race bits: Human 1, Orc 2, Dwarf 4, Night Elf 8, Undead 16, Tauren 32, Gnome 64, Troll 128 [V:src, bit formula; the id table is the 1.12 enum].

Values in the 28 [V:data]: the six letters 3100 / 3101 / 3102 / 3103 / 3104 / 3105 carry class 1 / 2 / 8 / 16 / 128 / 256 (Warrior, Paladin, Rogue, Priest, Mage, Warlock) and race 1 (Human only); 5623 is class 16 race 1 (MinLevel 5); 1598 is class 256 with race 77 = Human + Dwarf + Night Elf + Gnome. All other quests have 0/0 (anyone). The zone/sort column carries the class sorts for those quests (-81 Warrior, -141 Paladin, -162 Rogue, -262 Priest, -161 Mage, -61 Warlock) [V:data; the sort-to-class reading is the standard QuestSort table, only 61 = Warlock confirmed in source, the others [I]].

### 1.8 Levels, XP, money

- `MinLevel` gates taking (check 5). `QuestLevel` is only for colour/low-level logic and XP reduction. `QuestLevel -1` ("use the player's level") does not occur [V:data]. MinLevel is 1 for 18 quests, 2 for 6, 18, 3903, 3904, 3905, 5 for 5623, 20 for 344-347 [V:data].
- **XP:** `RewXP` is the direct XP number (e.g. 170 for Kobold Camp Cleanup). `Quest::XPValue(playerLevel)` returns the full value while the player is at most 5 levels above QuestLevel, then 80%, 60%, 40%, 20% at 6, 7, 8, 9 levels above and 10% from 10 levels above, rounded up; `RewardQuest` multiplies by `Rate.XP.Quest` (default 1) and only gives XP below the level cap [V:src]. At the cap the same slot pays `RewMoneyMaxLevel` copper instead (a 1.10+ rule), irrelevant here.
- Totals [V:data]: the eight level-1 Warrior quests (783, 5261, 7, 33, 3100, 15, 21, 54) give 1430 XP; the five level-2 ones (18, 6, 3903, 3904, 3905) give 1260; with 2158 (110) the 14 Warrior quests total 2800 XP. Money is tiny: only 7 (25 copper) and 15 (40 copper) pay coins. The XP needed for 1->2 is 400 in vanilla [I]; vMaNGOS's `player_xp_for_level` is not exported, so G4 must source its XP table separately [N].
- **No XP in G3 (open question 1):** with the player stuck at level 1, `SatisfyQuestLevel` rejects 18, 6, 3903, 3904 and 3905. Section 1.12 shows the effect.

### 1.9 patch rows

See 1.1: three quests have patch rows other than 0 (1598 -> 7, 7961 -> 9, 7962 -> 4). The importer's "highest patch <= 10" rule is exactly the loader's rule [V:src, V:data].

### 1.10 Flags and special flags that matter

QuestFlags values in the 28 [V:data]: **0** for 54, 346, 3100, 3101, 3102, 3103, 3104, 3105, 3905, 5805; **8** (SHARABLE: party sharing, irrelevant for a single-player game) for 17 quests; **10** (PARTY_ACCEPT 2 + SHARABLE 8) only for 7961. There is no EPIC, RAID, EXPLORATION, HIDDEN_REWARDS or AUTO_REWARDED flag [V:data; flag meanings V:src `QuestDef.h`]. SpecialFlags are 0 for all 28: none is REPEATABLE, none is flagged EXPLORATION_OR_EVENT [V:data]. Type is 0 and SuggestedPlayers 0 and RequiredSkill 0 for all [V:data]. `LimitTime` is 0 for all (no timed quest) [V:data]. Method 2 for all (none auto-complete, none disabled) [V:data]. None of the 28 is in `game_event_quest` (no seasonal quest) [V:data].

What the flags would do in vMaNGOS if a quest had them (none of the 28 has them, listed so the engine can ignore them knowingly) [V:src]: SpecialFlags REPEATABLE (1): after reward the status goes back to NONE and the NPC shows the repeatable "reward" mark; QuestFlags PARTY_ACCEPT (2): the other group members get a confirm box; SHARABLE (8): `Player::CanShareQuest`, quest sharing in a group; EPIC (0x20), RAID (0x40): only the raid flag matters (quest allowed inside a raid); HIDDEN_REWARDS (0x200): rewards only shown in the reward window; AUTO_REWARDED (0x400): rewarded automatically at completion, never shown in the log, only the first reward choice allowed; TIMED (derived from `LimitTime`): one timed quest at a time and a failure timer; Method 0 (auto-complete): no objectives, the ender shows the reward mark as soon as `CanTakeQuest` passes.

### 1.11 Dependency table of the 28 (derived by script from the manifest + the loader rules of 1.1)

Columns: own PrevQuestId; derived prevQuests (= own plus the quests that name this one in NextQuestId); derived prevChain (quests whose NextQuestInChain is this one); NextQuestInChain (auto-offer); ExclusiveGroup; whether the Human Warrior can take it. "giver outside" = the giver NPC has no spawn in the valley.

| id | title | MinLvl | class/race gate | PrevQuestId | prevQuests (derived) | prevChain (derived) | NextQuestInChain | ExclusiveGroup | Human Warrior |
|---|---|---|---|---|---|---|---|---|---|
| 6 | Bounty on Garrick Padfoot | 2 | - | 18 | 18 | - | - | - | yes - needs level 2; needs BOTH 18 and 33 rewarded (each-from-all) |
| 7 | Kobold Camp Cleanup | 1 | - | 783 | 783 | 783 | - | - | yes |
| 15 | Investigate Echo Ridge | 1 | - | 7 | 7 | - | 21 | - | yes |
| 18 | Brotherhood of Thieves | 2 | - | 783 | 783 | - | - | -18 (each-from-all) | yes - needs level 2 |
| 21 | Skirmish at Echo Ridge | 1 | - | 15 | 15 | 15 | - | - | yes |
| 33 | Wolves Across the Border | 1 | - | 5261 | 5261 | 5261 | - | -18 (each-from-all) | yes |
| 54 | Report to Goldshire | 1 | - | 21 | 21 | - | - | - | yes |
| 344 | Brother Paxton | 20 | - | 343 | 343 | - | 345 | - | no - giver NPC is outside the valley |
| 345 | Ink Supplies | 20 | - | 344 | 344 | 344 | 347 | - | no - needs the Stormwind quest 343/344 first and level 20 |
| 346 | Return to Kristoff | 20 | - | 347 | 347 | 347 | - | - | no - needs the Stormwind quest 343/344 first and level 20 |
| 347 | Rethban Ore | 20 | - | 345 | 345 | 345 | 346 | - | no - giver NPC is outside the valley |
| 783 | A Threat Within | 1 | - | - | - | - | 7 | - | yes |
| 1598 | The Stolen Tome | 1 | cls Warlock / race Human+Dwarf+Night Elf+Gnome | - | - | - | - | - | no - class Warlock only |
| 2158 | Rest and Relaxation | 1 | - | - | - | - | - | - | yes |
| 3100 | Simple Letter | 1 | cls Warrior / race Human | 7 | 7 | - | - | - | yes |
| 3101 | Consecrated Letter | 1 | cls Paladin / race Human | 7 | 7 | - | - | - | no - class Paladin only |
| 3102 | Encrypted Letter | 1 | cls Rogue / race Human | 7 | 7 | - | - | - | no - class Rogue only |
| 3103 | Hallowed Letter | 1 | cls Priest / race Human | 7 | 7 | - | - | - | no - class Priest only |
| 3104 | Glyphic Letter | 1 | cls Mage / race Human | 7 | 7 | - | - | - | no - class Mage only |
| 3105 | Tainted Letter | 1 | cls Warlock / race Human | 7 | 7 | - | - | - | no - class Warlock only |
| 3903 | Milly Osworth | 2 | - | - | 18,33 | - | 3904 | - | yes - needs level 2; needs BOTH 18 and 33 rewarded (each-from-all) |
| 3904 | Milly's Harvest | 2 | - | 3903 | 3903 | 3903 | 3905 | - | yes - needs level 2 |
| 3905 | Grape Manifest | 2 | - | 3904 | 3904 | 3904 | - | - | yes - needs level 2 |
| 5261 | Eagan Peltskinner | 1 | - | 783 | 783 | - | 33 | - | yes |
| 5623 | In Favor of the Light | 5 | cls Priest / race Human | - | - | - | 5624 | - | no - class Priest only |
| 5805 | Welcome! | 1 | - | - | - | - | - | - | no - no giver row (section 2.2) |
| 7961 | Waskily Wabbits! | 1 | - | - | - | - | - | - | no - no giver row (section 2.2) |
| 7962 | Wabbit Pelts | 1 | - | - | - | - | - | - | no - no giver row (section 2.2) |

### 1.12 What the Warrior sees, wave by wave (simulation of the vMaNGOS rules over the manifest)

Rule of the simulation: each wave takes everything `CanTakeQuest` allows whose giver NPC is spawned in the valley, then turns in every quest whose ender is spawned in the valley (instant objectives). [V:data + V:src logic implemented in a script]

- **Player stuck at level 1 (no XP in G3):** wave 1: take 783, 2158 -> 783 rewarded (2158 stays in the log, its ender is outside). Wave 2: 7, 5261. Wave 3: 15, 33, 3100. Wave 4: 21. Wave 5: 54 (stays in the log). **Never reachable: 6, 18, 3903, 3904, 3905** (MinLevel 2) plus the 14 other-class / Stormwind / orphan quests.
- **Level 2 or higher (or MinLevel ignored):** wave 1: 783, 2158. Wave 2: 7, 18, 5261. Wave 3: 15, 33, 3100. Wave 4: **6, 21, 3903** (6 and 3903 only now, because 33 became rewarded in wave 3). Wave 5: 54, 3904. Wave 6: 3905. Everything reachable is done after 6 waves; 54 and 2158 stay in the log.
- Unit-test vectors for the engine (same rules): rewarded {783} at level 1 -> takeable {7, 5261} (and 2158 always, 3100 not yet); rewarded {783} at level 2 -> {7, 18, 5261}; rewarded {783, 7} -> adds {15, 3100}; 15 current (not rewarded) -> 21 not takeable (needs 15 rewarded, and PrevChain forbids while 15 is current); rewarded {18} -> 3903 and 6 not takeable; rewarded {18, 33} -> both takeable; 18 or 33 in the log -> 3903 not takeable; a quest already in the log or finished -> not takeable (status check).

Reference logic for the engine (my formulation of the rules above, not source text):

    can_take(q, p):
      status(q) == NONE and p.level >= q.min_level
      and (q.req_classes == 0 or q.req_classes & bit(p.class)) and (q.req_races == 0 or q.req_races & bit(p.race))
      and prev_ok(q) and not (q.next_chain is current-or-complete in log) and no quest naming q as next_chain is current
      (exclusive-group > 0 check: no other group member INCOMPLETE/COMPLETE; groups <= 0 never block)
    prev_ok(q): q.prevQuests empty -> true; else some prerequisite p (abs id) that exists in the log with
      (p > 0 and rewarded) or (p < 0 and current), AND if that prerequisite's group is negative, every OTHER member of the
      group is in the same state (rewarded resp. current); if the check for the first qualifying prerequisite fails, the result is false.

---

## 2. Per-quest cards for the 28 quests

### 2.0 Index of the 28 quests (QuestLevel/MinLevel, giver, ender, objective, rewards, XP, money) [V:data]

| id | title | QL/Min | giver | ender | objective | rewards | XP | money |
|---|---|---|---|---|---|---|---|---|
| 6 | Bounty on Garrick Padfoot | 5/2 | Deputy Willem | Deputy Willem | collect 1 x Garrick's Head | choose 1 of 3: Tapered Pants, Layered Tunic, Ensign Cloak | 335 | - |
| 7 | Kobold Camp Cleanup | 2/1 | Marshal McBride | Marshal McBride | kill 10 x Kobold Vermin | - | 170 | 0g 0s 25c |
| 15 | Investigate Echo Ridge | 3/1 | Marshal McBride | Marshal McBride | kill 10 x Kobold Worker | - | 250 | 0g 0s 40c |
| 18 | Brotherhood of Thieves | 4/2 | Deputy Willem | Deputy Willem | collect 12 x Red Burlap Bandana | choose 1 of 5: Militia Dagger, Militia Hammer, Militia Shortsword, Militia Warhammer, Militia Quarterstaff | 355 | - |
| 21 | Skirmish at Echo Ridge | 5/1 | Marshal McBride | Marshal McBride | kill 12 x Kobold Laborer | choose 1 of 3: Outfitter Belt, Outfitter Boots, Outfitter Gloves | 450 | - |
| 33 | Wolves Across the Border | 2/1 | Eagan Peltskinner | Eagan Peltskinner | collect 8 x Tough Wolf Meat | choose 1 of 2: Soft Fur-lined Shoes, Wolfskin Bracers | 170 | - |
| 54 | Report to Goldshire | 5/1 | Marshal McBride | Marshal Dughan | carry Marshal McBride's Documents | fixed: Pikeman Shield | 225 | - |
| 344 | Brother Paxton | 24/20 | Milton Sheaf | Brother Paxton | talk only | - | 485 | - |
| 345 | Ink Supplies | 24/20 | Brother Paxton | Foreman Oslow | talk only | - | 485 | - |
| 346 | Return to Kristoff | 24/20 | Brother Paxton | Brother Kristoff | carry Book: Stresses of Iron | fixed: Wandering Boots | 1950 | - |
| 347 | Rethban Ore | 24/20 | Foreman Oslow | Brother Paxton | collect 5 x Rethban Ore | - | 970 | - |
| 783 | A Threat Within | 1/1 | Deputy Willem | Marshal McBride | talk only | - | 40 | - |
| 1598 | The Stolen Tome | 4/1 | Drusilla La Salle | Drusilla La Salle | collect 1 x Powers of the Void | - | 355 | - |
| 2158 | Rest and Relaxation | 5/1 | Falkhaan Isenstrider | Innkeeper Farley | talk only | choose 1 of 2: Small Pumpkin x5, Refreshing Spring Water x5 | 110 | - |
| 3100 | Simple Letter | 1/1 | Marshal McBride | Llane Beshere | carry Simple Letter | - | 40 | - |
| 3101 | Consecrated Letter | 1/1 | Marshal McBride | Brother Sammuel | carry Consecrated Letter | - | 40 | - |
| 3102 | Encrypted Letter | 1/1 | Marshal McBride | Jorik Kerridan | carry Encrypted Letter | - | 40 | - |
| 3103 | Hallowed Letter | 1/1 | Marshal McBride | Priestess Anetta | carry Hallowed Letter | - | 40 | - |
| 3104 | Glyphic Letter | 1/1 | Marshal McBride | Khelden Bremen | carry Glyphic Letter | - | 40 | - |
| 3105 | Tainted Letter | 1/1 | Marshal McBride | Drusilla La Salle | carry Tainted Letter | - | 40 | - |
| 3903 | Milly Osworth | 4/2 | Deputy Willem | Milly Osworth | talk only | - | 35 | - |
| 3904 | Milly's Harvest | 4/2 | Milly Osworth | Milly Osworth | collect 8 x Milly's Harvest | - | 180 | - |
| 3905 | Grape Manifest | 4/2 | Milly Osworth | Brother Neals | carry Grape Manifest | choose 1 of 2: Wine-stained Cloak, Latched Belt | 355 | - |
| 5261 | Eagan Peltskinner | 2/1 | Deputy Willem | Eagan Peltskinner | talk only | - | 85 | - |
| 5623 | In Favor of the Light | 4/5 | Priestess Anetta | Priestess Josetta | talk only | - | 90 | - |
| 5805 | Welcome! | 1/1 | NONE | Merissa Stilwell | carry Northshire Gift Voucher | choose 1 of 3: Diablo Stone, Panda Collar, Zergling Leash | 0 | - |
| 7961 | Waskily Wabbits! | 1/1 | NONE | NONE | kill 60 x Polymorph Clone; kill 60 x The Good Rabbit | fixed: Rabbit's Foot x5, Rabbit Crate (Snowshoe) | 20 | 0g 0s 4c |
| 7962 | Wabbit Pelts | 1/1 | NONE | NONE | collect 3 x LeCraft Rabbit Pelt | fixed: 22 Pound Lobster | 100 | 0g 0s 30c |

### 2.0b NPCs that give or take the 28 quests (positions are WoW coordinates, +X north, +Y west, +Z up; areas are the importer's `area` column) [V:data]

| entry | name | subname | npc_flags | position (first spawn) | gives | takes |
|---|---|---|---|---|---|---|
| 196 | Eagan Peltskinner | - | 2 | (-8869.22, -163.24, 80.97) Northshire Valley | 33 | 33, 5261 |
| 197 | Marshal McBride | - | 3 | (-8902.59, -162.61, 82.02) Northshire Valley | 7, 15, 21, 54, 3100, 3101, 3102, 3103, 3104, 3105 | 7, 15, 21, 783 |
| 198 | Khelden Bremen | Mage Trainer | 19 | (-8851.57, -188.23, 89.50) Northshire Valley | - | 3104 |
| 240 | Marshal Dughan | - | 2 | not in the valley | - | 54 |
| 295 | Innkeeper Farley | Innkeeper | 135 | not in the valley | - | 2158 |
| 341 | Foreman Oslow | - | 2 | not in the valley | 347 | 345 |
| 375 | Priestess Anetta | Priest Trainer | 19 | (-8853.59, -193.34, 82.12) Northshire Valley | 5623 | 3103 |
| 377 | Priestess Josetta | Priest Trainer | 19 | not in the valley | - | 5623 |
| 459 | Drusilla La Salle | Warlock Trainer | 19 | (-8926.74, -195.59, 80.77) Northshire Valley | 1598 | 1598, 3105 |
| 823 | Deputy Willem | - | 2 | (-8933.54, -136.52, 83.45) Northshire Valley | 6, 18, 783, 3903, 5261 | 6, 18 |
| 911 | Llane Beshere | Warrior Trainer | 19 | (-8918.36, -208.41, 82.31) Northshire Valley | - | 3100 |
| 915 | Jorik Kerridan | Rogue Trainer | 19 | (-8863.47, -210.91, 80.75) Northshire Valley | - | 3102 |
| 925 | Brother Sammuel | Paladin Trainer | 19 | (-8914.57, -215.02, 82.30) Northshire Valley | - | 3101 |
| 951 | Brother Paxton | Librarian | 2 | (-8874.87, -186.33, 82.12) Northshire Valley | 345, 346 | 344, 347 |
| 952 | Brother Neals | - | 2 | (-8902.13, -181.65, 113.24) Northshire Valley | - | 3905 |
| 1440 | Milton Sheaf | Librarian | 2 | not in the valley | 344 | - |
| 1444 | Brother Kristoff | - | 2 | not in the valley | - | 346 |
| 6774 | Falkhaan Isenstrider | - | 2 | (-9044.56, -45.98, 88.42) Northshire Valley | 2158 | - |
| 9296 | Milly Osworth | - | 2 | (-8850.29, -224.03, 81.78) Northshire Valley | 3904, 3905 | 3903, 3904 |
| 11940 | Merissa Stilwell | - | 2 | (-8908.31, -108.52, 81.93) Northshire Valley | - | 5805 |

npc_flags: 2 = quest giver, 3 = gossip + quest giver (McBride: his greeting text lives in `npc_text` / `gossip_menu`, which are not exported [N]), 19 = gossip + quest giver + trainer (class trainers), 135 = gossip + quest giver + vendor + innkeeper [V:src flag values].

### 2.1 The cards

Field notes: "Giver"/"Ender" come from `start` / `end` rows (creature relations; there are no object relations among the 28 [V:data]). "Links" shows the raw columns and the derived ones (section 1.1). XP is the stored `RewXP`. All text lengths are in characters, the verbatim texts are on the cited line of `cache/world/northshire.txt` (escape rules of `tsv.rs`; a newline inside a text is a backslash + newline).

### 6 - Bounty on Garrick Padfoot

Gist (paraphrase): Willem wants the head of the bandit Garrick Padfoot, hiding in a shack near the vineyards east of the Abbey behind a gang of thugs.

- Level: QuestLevel 5, MinLevel 2, MaxLevel not exported [N]. Zone/sort 9 = Northshire Valley (area 9). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Deputy Willem (823) at (-8933.54, -136.52, 83.45) facing 1.972, Northshire Valley, respawn 270 s [V:data]
- Ender: Deputy Willem (823) at (-8933.54, -136.52, 83.45) facing 1.972, Northshire Valley, respawn 270 s [V:data]
- Links: PrevQuestId 18; derived prevQuests [18] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Required item: Garrick's Head (182) [Quest class 12 / sub 0, common]
  - source: Garrick Padfoot (103): 100% QUEST drop (needs the item) - 1 spawns (1 in Northshire Vineyards), box x -9056..-9056 y -461..-461, movement/wander stands: 1, respawn 270 s
- Reward choice (pick 1): Tapered Pants (6076) [armor class 4 / sub 1 = Cloth, inv 7 = Legs, armor 9, common]; Layered Tunic (60) [armor class 4 / sub 2 = Leather, inv 5 = Chest, armor 33, common]; Ensign Cloak (3070) [armor class 4 / sub 1 = Cloth, inv 16 = Back, armor 5, common]
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 335 (direct XP, see 1.8). RewMoneyMaxLevel 210 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 569 of `cache/world/northshire.txt`: Details 332 chars ($B x2, $N x1); Objectives 77 chars (none); OfferRewardText 84 chars (none); RequestItemsText 67 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Needs item 182 Garrick's Head (unique, max_count 1; item flag 0x800 = party-loot flag in vMaNGOS ItemPrototype [I]). Garrick drops it with chance -100 = always, but only while the quest still needs it [V:src LootStoreItem + AllowedForPlayer].
- Note: Prerequisite quirk: PrevQuestId 18 is a member of ExclusiveGroup -18, so vMaNGOS requires 33 to be rewarded as well (see section 1.5). Giver and ender are the same NPC (Willem).

### 7 - Kobold Camp Cleanup

Gist (paraphrase): McBride sends the recruit to thin out the kobold vermin in the woods north of Northshire.

- Level: QuestLevel 2, MinLevel 1, MaxLevel not exported [N]. Zone/sort 9 = Northshire Valley (area 9). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Marshal McBride (197) at (-8902.59, -162.61, 82.02) facing 1.988, Northshire Valley, respawn 285 s [V:data]
- Ender: Marshal McBride (197) at (-8902.59, -162.61, 82.02) facing 1.988, Northshire Valley, respawn 285 s [V:data]
- Links: PrevQuestId 783; derived prevQuests [783] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests [783]; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Required kill: 10 x Kobold Vermin (6); 33 spawns (33 in Northshire Valley), box x -8811..-8750 y -258..-93, movement/wander wanders 5 yd: 25, stands: 8, respawn 270 s
- Item rewards: none
- Money: 0g 0s 25c (25 copper) (RewOrReqMoney 25; positive = reward). XP: RewXP 170 (direct XP, see 1.8). RewMoneyMaxLevel 90 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 570 of `cache/world/northshire.txt`: Details 218 chars ($N x1); Objectives 54 chars (none); OfferRewardText 209 chars ($B x2); RequestItemsText 68 chars ($N x1); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: No NextQuestInChain: turning it in does not auto-offer anything; 15 and 3100 simply get a "!" at McBride afterwards.
- Note: Kill credit counts a kill wherever it happens; there is no area restriction in the data.

### 15 - Investigate Echo Ridge

Gist (paraphrase): McBride reports a larger kobold camp at the Echo Ridge Mine and wants the workers there killed; he pays on turn-in and hints at another task.

- Level: QuestLevel 3, MinLevel 1, MaxLevel not exported [N]. Zone/sort 9 = Northshire Valley (area 9). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Marshal McBride (197) at (-8902.59, -162.61, 82.02) facing 1.988, Northshire Valley, respawn 285 s [V:data]
- Ender: Marshal McBride (197) at (-8902.59, -162.61, 82.02) facing 1.988, Northshire Valley, respawn 285 s [V:data]
- Links: PrevQuestId 7; derived prevQuests [7] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 21 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Required kill: 10 x Kobold Worker (257); 43 spawns (20 in Northshire Valley, 23 in Echo Ridge Mine), box x -8824..-8667 y -275..-69, movement/wander wanders 5 yd: 32, path: 1, stands: 8, wanders 7 yd: 2, respawn 270 s
- Item rewards: none
- Money: 0g 0s 40c (40 copper) (RewOrReqMoney 40; positive = reward). XP: RewXP 250 (direct XP, see 1.8). RewMoneyMaxLevel 150 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 571 of `cache/world/northshire.txt`: Details 283 chars ($B x2, $N x1); Objectives 60 chars (none); OfferRewardText 211 chars (none); RequestItemsText 53 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: NextQuestInChain 21: on turn-in McBride offers 21 in the same window (Player::GetNextQuest, HandleQuestgiverChooseRewardOpcode) [V:src].
- Note: The 40 copper is paid on turn-in; the offer-reward text presents it as payment.

### 18 - Brotherhood of Thieves

Gist (paraphrase): Willem wants the red bandanas of the Defias Brotherhood thieves seen across the river to the east; the reward is a weapon of the player's choice.

- Level: QuestLevel 4, MinLevel 2, MaxLevel not exported [N]. Zone/sort 9 = Northshire Valley (area 9). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Deputy Willem (823) at (-8933.54, -136.52, 83.45) facing 1.972, Northshire Valley, respawn 270 s [V:data]
- Ender: Deputy Willem (823) at (-8933.54, -136.52, 83.45) facing 1.972, Northshire Valley, respawn 270 s [V:data]
- Links: PrevQuestId 783; derived prevQuests [783] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 3903; ExclusiveGroup -18; NextQuestInChain 0 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Required item: Red Burlap Bandana (752) x12 [Quest class 12 / sub 0, common]
  - source: Defias Thug (38): 80% QUEST drop (needs the item) - 77 spawns (31 in Northshire Valley, 46 in Northshire Vineyards), box x -9148..-8869 y -458..-234, movement/wander stands: 49, wanders 5 yd: 19, path: 9, respawn 270 s
- Reward choice (pick 1): Militia Dagger (2224) [weapon class 2 / sub 15 = Dagger, inv 13 = One-Hand, dmg 2-4, delay 1500 ms, common]; Militia Hammer (5580) [weapon class 2 / sub 4 = 1H Mace, inv 21 = Main Hand, dmg 3-6, delay 2300 ms, common]; Militia Shortsword (1161) [weapon class 2 / sub 7 = 1H Sword, inv 21 = Main Hand, dmg 3-7, delay 2500 ms, common]; Militia Warhammer (5579) [weapon class 2 / sub 5 = 2H Mace, inv 17 = Two-Hand, dmg 6-9, delay 2900 ms, common]; Militia Quarterstaff (1159) [weapon class 2 / sub 10 = Staff, inv 17 = Two-Hand, dmg 6-9, delay 2800 ms, common]
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 355 (direct XP, see 1.8). RewMoneyMaxLevel 210 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 572 of `cache/world/northshire.txt`: Details 296 chars ($B x2); Objectives 75 chars (none); OfferRewardText 74 chars (none); RequestItemsText 44 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).

### 21 - Skirmish at Echo Ridge

Gist (paraphrase): McBride sends the player back into Echo Ridge Mine to purge the kobold laborers.

- Level: QuestLevel 5, MinLevel 1, MaxLevel not exported [N]. Zone/sort 9 = Northshire Valley (area 9). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Marshal McBride (197) at (-8902.59, -162.61, 82.02) facing 1.988, Northshire Valley, respawn 285 s [V:data]
- Ender: Marshal McBride (197) at (-8902.59, -162.61, 82.02) facing 1.988, Northshire Valley, respawn 285 s [V:data]
- Links: PrevQuestId 15; derived prevQuests [15] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests [15]; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Required kill: 12 x Kobold Laborer (80); 17 spawns (17 in Echo Ridge Mine), box x -8672..-8574 y -174..-111, movement/wander wanders 7 yd: 10, stands: 7, respawn 270 s
- Reward choice (pick 1): Outfitter Belt (2186) [armor class 4 / sub 2 = Leather, inv 6 = Waist, armor 19, common]; Outfitter Boots (2691) [armor class 4 / sub 3 = Mail, inv 8 = Feet, armor 46, common]; Outfitter Gloves (11192) [armor class 4 / sub 1 = Cloth, inv 10 = Hands, armor 6, common]
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 450 (direct XP, see 1.8). RewMoneyMaxLevel 270 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Scripts: StartScript 0, CompleteScript 21 (rows of `quest_start_scripts` / `quest_end_scripts`; tables not exported, content [N])
- Texts [V:data], line 573 of `cache/world/northshire.txt`: Details 254 chars ($B x2, $N x1); Objectives 76 chars (none); OfferRewardText 191 chars (none); RequestItemsText 98 chars ($N x1); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: CompleteScript 21 runs at turn-in (table `quest_end_scripts`, not exported: content [N]).
- Note: No NextQuestInChain; 54 (prev 21) just becomes available at McBride.

### 33 - Wolves Across the Border

Gist (paraphrase): Eagan Peltskinner wants tough wolf meat from the timber and young wolves and pays with an item.

- Level: QuestLevel 2, MinLevel 1, MaxLevel not exported [N]. Zone/sort 9 = Northshire Valley (area 9). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Eagan Peltskinner (196) at (-8869.22, -163.24, 80.97) facing 0.960, Northshire Valley, respawn 270 s [V:data]
- Ender: Eagan Peltskinner (196) at (-8869.22, -163.24, 80.97) facing 0.960, Northshire Valley, respawn 270 s [V:data]
- Links: PrevQuestId 5261; derived prevQuests [5261] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 3903; ExclusiveGroup -18; NextQuestInChain 0 (auto-offer); derived prevChainQuests [5261]; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Required item: Tough Wolf Meat (750) x8 [Quest class 12 / sub 0, common]
  - source: Timber Wolf (69): 80% QUEST drop (needs the item) - 36 spawns (35 in Northshire Valley, 1 in Echo Ridge Mine), box x -9080..-8719 y -307..-33, movement/wander wanders 5 yd: 34, stands: 2, respawn 270 s
  - source: Young Wolf (299): 80% QUEST drop (needs the item) - 37 spawns (37 in Northshire Valley), box x -9025..-8800 y -267..-41, movement/wander wanders 10 yd: 37, respawn 270 s
  - Also in the whole database: Ragged Timber Wolf (704) and Ragged Young Wolf (705) carry the same -80% row; neither is spawned in the valley [V:data tsv].
- Reward choice (pick 1): Soft Fur-lined Shoes (80) [armor class 4 / sub 1 = Cloth, inv 8 = Feet, armor 7, common]; Wolfskin Bracers (6070) [armor class 4 / sub 2 = Leather, inv 9 = Wrist, armor 15, common]
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 170 (direct XP, see 1.8). RewMoneyMaxLevel 90 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 574 of `cache/world/northshire.txt`: Details 272 chars ($B x2); Objectives 80 chars (none); OfferRewardText 120 chars ($B x2); RequestItemsText 63 chars ($N x1); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Eagan offers 33 straight after 5261 is handed in (5261 has NextQuestInChain 33). ExclusiveGroup -18 is "each-from-all", not exclusive: 18 and 33 can be active together (section 1.5).

### 54 - Report to Goldshire

Gist (paraphrase): McBride names the player protector of Elwynn Forest and hands over papers for Marshal Dughan in Goldshire; Dughan's reply grants Acting Deputy status.

- Level: QuestLevel 5, MinLevel 1, MaxLevel not exported [N]. Zone/sort 9 = Northshire Valley (area 9). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 0, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Marshal McBride (197) at (-8902.59, -162.61, 82.02) facing 1.988, Northshire Valley, respawn 285 s [V:data]
- Ender: Marshal Dughan (240) - NO spawn in the manifest (outside the valley) [V:data]
- Links: PrevQuestId 21; derived prevQuests [21] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item (given on accept, unless the player already holds the count: CanGiveQuestSourceItemIfNeed): Marshal McBride's Documents (745) [Quest class 12 / sub 0, common]; it is also the required item, so the quest is COMPLETE immediately after accept [V:src] [V:data]
- Required item: Marshal McBride's Documents (745) [Quest class 12 / sub 0, common]
  - source: only the quest's own source item (no loot row carries it) [V:data tsv]
- Fixed rewards: Pikeman Shield (6078) [armor class 4 / sub 6 = Shield, inv 14 = Shield, armor 55, block 1, common]
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 225 (direct XP, see 1.8). RewMoneyMaxLevel 120 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Scripts: StartScript 54, CompleteScript 0 (rows of `quest_start_scripts` / `quest_end_scripts`; tables not exported, content [N])
- Texts [V:data], line 575 of `cache/world/northshire.txt`: Details 317 chars ($B x2, $N x1, $c x1); Objectives 64 chars (none); OfferRewardText 304 chars ($B x4); RequestItemsText 162 chars ($B x2); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Source item 745 equals the required item 745: it is given on accept and the quest is COMPLETE at once (HandleQuestgiverAcceptQuestOpcode calls CompleteQuest when CanCompleteQuest) [V:src]; nothing else to do except reach the ender.
- Note: The ender (Marshal Dughan, Goldshire) is NOT in the valley: see section 2.3. StartScript 54 (`quest_start_scripts`, not exported [N]).
- Note: The fixed reward Pikeman Shield (6078) is the only shield any of the 28 quests gives; the player can never get it unless 54 can be turned in.

### 344 - Brother Paxton

Gist (paraphrase): Stormwind chain: Milton Sheaf wonders where his borrowed book went and sends the player to Brother Paxton in the Abbey library.

- Level: QuestLevel 24, MinLevel 20, MaxLevel not exported [N]. Zone/sort 1519 = Stormwind City (area 1519). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Milton Sheaf (1440, Librarian) - NO spawn in the manifest (outside the valley) [V:data]
- Ender: Brother Paxton (951, Librarian) at (-8874.87, -186.33, 82.12) facing 1.309, Northshire Valley, respawn 375 s [V:data]
- Links: PrevQuestId 343; derived prevQuests [343] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 345 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Objectives: none (a talk quest: no counters, completable as soon as it is accepted) [V:data, logic V:src]
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 485 (direct XP, see 1.8). RewMoneyMaxLevel 300 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Point hint: map 0, X -8876.67, Y -184.318, PointOpt 1 [V:data]
- Texts [V:data], line 576 of `cache/world/northshire.txt`: Details 502 chars ($B x4); Objectives 50 chars (none); OfferRewardText 171 chars (none); RequestItemsText 0 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Whole Stormwind chain 343 (Kristoff) -> 344 (Milton Sheaf) -> 345 -> 347 -> 346 needs NPCs that are not in the valley; MinLevel 20. PointX/PointY = Paxton's spot with PointOpt 1 (client map hint, [I]).

### 345 - Ink Supplies

Gist (paraphrase): Brother Paxton has no ink left and sends the player to Foreman Oslow in Lakeshire for Rethban iron ore.

- Level: QuestLevel 24, MinLevel 20, MaxLevel not exported [N]. Zone/sort 1519 = Stormwind City (area 1519). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Brother Paxton (951, Librarian) at (-8874.87, -186.33, 82.12) facing 1.309, Northshire Valley, respawn 375 s [V:data]
- Ender: Foreman Oslow (341) - NO spawn in the manifest (outside the valley) [V:data]
- Links: PrevQuestId 344; derived prevQuests [344] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 347 (auto-offer); derived prevChainQuests [344]; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Objectives: none (a talk quest: no counters, completable as soon as it is accepted) [V:data, logic V:src]
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 485 (direct XP, see 1.8). RewMoneyMaxLevel 300 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 577 of `cache/world/northshire.txt`: Details 459 chars ($B x4); Objectives 71 chars (none); OfferRewardText 199 chars (none); RequestItemsText 0 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Giver Paxton is in the valley but the quest needs 344 first; ender Foreman Oslow is in Lakeshire.

### 346 - Return to Kristoff

Gist (paraphrase): Paxton hands back the finished-with book for Brother Kristoff in Stormwind's Cathedral Square.

- Level: QuestLevel 24, MinLevel 20, MaxLevel not exported [N]. Zone/sort 1519 = Stormwind City (area 1519). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 0, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Brother Paxton (951, Librarian) at (-8874.87, -186.33, 82.12) facing 1.309, Northshire Valley, respawn 375 s [V:data]
- Ender: Brother Kristoff (1444) - NO spawn in the manifest (outside the valley) [V:data]
- Links: PrevQuestId 347; derived prevQuests [347] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests [347]; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item (given on accept, unless the player already holds the count: CanGiveQuestSourceItemIfNeed): Book: Stresses of Iron (2795) [Quest class 12 / sub 0, common]; it is also the required item, so the quest is COMPLETE immediately after accept [V:src] [V:data]
- Required item: Book: Stresses of Iron (2795) [Quest class 12 / sub 0, common]
  - source: only the quest's own source item (no loot row carries it) [V:data tsv]
- Fixed rewards: Wandering Boots (6095) [armor class 4 / sub 1 = Cloth, inv 8 = Feet, armor 26, uncommon]
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 1950 (direct XP, see 1.8). RewMoneyMaxLevel 1200 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Scripts: StartScript 0, CompleteScript 346 (rows of `quest_start_scripts` / `quest_end_scripts`; tables not exported, content [N])
- Texts [V:data], line 578 of `cache/world/northshire.txt`: Details 176 chars (none); Objectives 92 chars (none); OfferRewardText 93 chars ($N x1); RequestItemsText 31 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Source item 2795 = required item 2795: complete at once on accept. Ender Kristoff is in Stormwind. CompleteScript 346 [N]. Reward Wandering Boots (uncommon).

### 347 - Rethban Ore

Gist (paraphrase): Foreman Oslow refuses to supply ore; the player must fetch 5 Rethban Ore himself for Brother Paxton.

- Level: QuestLevel 24, MinLevel 20, MaxLevel not exported [N]. Zone/sort 1519 = Stormwind City (area 1519). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Foreman Oslow (341) - NO spawn in the manifest (outside the valley) [V:data]
- Ender: Brother Paxton (951, Librarian) at (-8874.87, -186.33, 82.12) facing 1.309, Northshire Valley, respawn 375 s [V:data]
- Links: PrevQuestId 345; derived prevQuests [345] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 346 (auto-offer); derived prevChainQuests [345]; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Required item: Rethban Ore (2798) x5 [Miscellaneous class 15 / sub 0, common]
  - Whole database only (nothing in the valley): Redridge Drudger (580) 33.83% normal drop, Copper Vein loot table 1735 (object 2055) 40%, Tin Vein table 1736 (object 2054) 2% [V:data tsv].
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 970 (direct XP, see 1.8). RewMoneyMaxLevel 570 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 579 of `cache/world/northshire.txt`: Details 510 chars ($B x4); Objectives 87 chars (none); OfferRewardText 117 chars ($B x2, $N x1); RequestItemsText 28 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Needs 5 Rethban Ore which no creature or node in the valley carries (global sources listed in the item line).

### 783 - A Threat Within

Gist (paraphrase): Deputy Willem welcomes the volunteer and tells him to report to his superior, Marshal McBride, inside the Abbey.

- Level: QuestLevel 1, MinLevel 1, MaxLevel not exported [N]. Zone/sort 9 = Northshire Valley (area 9). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Deputy Willem (823) at (-8933.54, -136.52, 83.45) facing 1.972, Northshire Valley, respawn 270 s [V:data]
- Ender: Marshal McBride (197) at (-8902.59, -162.61, 82.02) facing 1.988, Northshire Valley, respawn 285 s [V:data]
- Links: PrevQuestId 0; derived prevQuests [] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 7 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Objectives: none (a talk quest: no counters, completable as soon as it is accepted) [V:data, logic V:src]
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 40 (direct XP, see 1.8). RewMoneyMaxLevel 30 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 580 of `cache/world/northshire.txt`: Details 474 chars ($B x6, $c x1); Objectives 27 chars (none); OfferRewardText 363 chars ($B x6, $N x1); RequestItemsText 0 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: No objectives: CanCompleteQuest passes at once, so the quest is COMPLETE as soon as it is accepted and McBride shows "?" [V:src logic, I for the no-hidden-check part].
- Note: NextQuestInChain 7: McBride offers 7 in the turn-in window.

### 1598 - The Stolen Tome

Gist (paraphrase): Warlock-only: Drusilla La Salle wants the stolen book "Powers of the Void", hidden in the Defias tent in the vineyards, as the test before she teaches imp summoning.

- Level: QuestLevel 4, MinLevel 1, MaxLevel not exported [N]. Zone/sort -61 = sort: Warlock. Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 7. [V:data]
- Class mask 256 (Warlock), race mask 77 (Human+Dwarf+Night Elf+Gnome). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Drusilla La Salle (459, Warlock Trainer) at (-8926.74, -195.59, 80.77) facing 2.793, Northshire Valley, respawn 270 s [V:data]
- Ender: Drusilla La Salle (459, Warlock Trainer) at (-8926.74, -195.59, 80.77) facing 2.793, Northshire Valley, respawn 270 s [V:data]
- Links: PrevQuestId 0; derived prevQuests [] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Required item: Powers of the Void (6785) [Quest class 12 / sub 0, common]
  - source: object Stolen Books (83763), loot table 4165: 100% QUEST drop, 1 spawns in the valley
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 355 (direct XP, see 1.8). RewMoneyMaxLevel 210 copper (paid instead of XP at max level; irrelevant here). RewSpell 688, RewSpellCast 7763. [V:data]
- Texts [V:data], line 581 of `cache/world/northshire.txt`: Details 507 chars ($b x8, $n x1); Objectives 54 chars (none); OfferRewardText 339 chars ($b x2); RequestItemsText 117 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Class Warlock, races 77 = Human+Dwarf+Night Elf+Gnome. Item 6785 comes from the chest object 83763 "Stolen Books" (consumable 0), 1 spawn at (-8957.04, -432.75, 64.72). Rewards the spell 688 (RewSpell) and casts 7763 on completion (RewSpellCast). `patch` 7 row.

### 2158 - Rest and Relaxation

Gist (paraphrase): Falkhaan recommends the Lion's Pride Inn and sends the player to his friend Innkeeper Farley in Goldshire.

- Level: QuestLevel 5, MinLevel 1, MaxLevel not exported [N]. Zone/sort 12 = Elwynn Forest (area 12). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Falkhaan Isenstrider (6774) at (-9044.56, -45.98, 88.42) facing 3.089, Northshire Valley, respawn 270 s [V:data]
- Ender: Innkeeper Farley (295, Innkeeper) - NO spawn in the manifest (outside the valley) [V:data]
- Links: PrevQuestId 0; derived prevQuests [] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Objectives: none (a talk quest: no counters, completable as soon as it is accepted) [V:data, logic V:src]
- Reward choice (pick 1): Small Pumpkin (4656) x5 [Consumable class 0 / sub 0, common]; Refreshing Spring Water (159) x5 [Consumable class 0 / sub 0, common]
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 110 (direct XP, see 1.8). RewMoneyMaxLevel 60 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 582 of `cache/world/northshire.txt`: Details 384 chars ($B x4); Objectives 52 chars (none); OfferRewardText 194 chars ($B x2); RequestItemsText 0 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Talk quest, complete at accept. Reward is a choice of two stacks of 5 consumables. The giver stands at the valley's southern edge (area 9).

### 3100 - Simple Letter

Gist (paraphrase): McBride's aide hands over a sealed letter from the warrior trainer Llane Beshere; the offer-reward text is Llane's own talk.

- Level: QuestLevel 1, MinLevel 1, MaxLevel not exported [N]. Zone/sort -81 = sort: Warrior. Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 1 (Warrior), race mask 1 (Human). Flags 0, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Marshal McBride (197) at (-8902.59, -162.61, 82.02) facing 1.988, Northshire Valley, respawn 285 s [V:data]
- Ender: Llane Beshere (911, Warrior Trainer) at (-8918.36, -208.41, 82.31) facing 6.056, Northshire Valley, respawn 270 s [V:data]
- Links: PrevQuestId 7; derived prevQuests [7] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item (given on accept, unless the player already holds the count: CanGiveQuestSourceItemIfNeed): Simple Letter (9542) [Quest class 12 / sub 0, common]; it is also the required item, so the quest is COMPLETE immediately after accept [V:src] [V:data]
- Required item: Simple Letter (9542) [Quest class 12 / sub 0, common]
  - source: only the quest's own source item (no loot row carries it) [V:data tsv]
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 40 (direct XP, see 1.8). RewMoneyMaxLevel 30 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 583 of `cache/world/northshire.txt`: Details 271 chars ($N x1); Objectives 70 chars (none); OfferRewardText 361 chars ($B x2, $C x1); RequestItemsText 170 chars ($B x2, $N x1); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Source item = required item: complete at accept; Llane Beshere (911) stands in the Abbey. Race mask 1 (Human) and class mask 1 (Warrior) both match the planned Human Warrior.
- Note: The "!" appears at McBride only after 7 is rewarded (PrevQuestId 7).

### 3101 - Consecrated Letter

Gist (paraphrase): Same pattern as 3100 for the paladin trainer Brother Sammuel.

- Level: QuestLevel 1, MinLevel 1, MaxLevel not exported [N]. Zone/sort -141 = sort: Paladin. Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 2 (Paladin), race mask 1 (Human). Flags 0, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Marshal McBride (197) at (-8902.59, -162.61, 82.02) facing 1.988, Northshire Valley, respawn 285 s [V:data]
- Ender: Brother Sammuel (925, Paladin Trainer) at (-8914.57, -215.02, 82.30) facing 1.204, Northshire Valley, respawn 270 s [V:data]
- Links: PrevQuestId 7; derived prevQuests [7] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item (given on accept, unless the player already holds the count: CanGiveQuestSourceItemIfNeed): Consecrated Letter (9570) [Quest class 12 / sub 0, common]; it is also the required item, so the quest is COMPLETE immediately after accept [V:src] [V:data]
- Required item: Consecrated Letter (9570) [Quest class 12 / sub 0, common]
  - source: only the quest's own source item (no loot row carries it) [V:data tsv]
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 40 (direct XP, see 1.8). RewMoneyMaxLevel 30 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 584 of `cache/world/northshire.txt`: Details 281 chars ($N x1); Objectives 77 chars (none); OfferRewardText 480 chars ($B x2, $c x1); RequestItemsText 335 chars ($gbrother:sister; x1); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Class gate: mask 2 = Paladin; the Human Warrior never sees it. Source item = required item (complete at accept); the trainer ender is in the Abbey.

### 3102 - Encrypted Letter

Gist (paraphrase): Same pattern as 3100 for the rogue trainer Jorik Kerridan (behind the Abbey).

- Level: QuestLevel 1, MinLevel 1, MaxLevel not exported [N]. Zone/sort -162 = sort: Rogue. Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 8 (Rogue), race mask 1 (Human). Flags 0, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Marshal McBride (197) at (-8902.59, -162.61, 82.02) facing 1.988, Northshire Valley, respawn 285 s [V:data]
- Ender: Jorik Kerridan (915, Rogue Trainer) at (-8863.47, -210.91, 80.75) facing 4.416, Northshire Valley, respawn 270 s [V:data]
- Links: PrevQuestId 7; derived prevQuests [7] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item (given on accept, unless the player already holds the count: CanGiveQuestSourceItemIfNeed): Encrypted Letter (9555) [Quest class 12 / sub 0, common]; it is also the required item, so the quest is COMPLETE immediately after accept [V:src] [V:data]
- Required item: Encrypted Letter (9555) [Quest class 12 / sub 0, common]
  - source: only the quest's own source item (no loot row carries it) [V:data tsv]
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 40 (direct XP, see 1.8). RewMoneyMaxLevel 30 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 585 of `cache/world/northshire.txt`: Details 271 chars ($N x1); Objectives 92 chars (none); OfferRewardText 494 chars ($B x2, $N x1, $gman:woman; x1); RequestItemsText 281 chars ($B x2); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Class gate: mask 8 = Rogue; the Human Warrior never sees it. Source item = required item (complete at accept); the trainer ender is in the Abbey.

### 3103 - Hallowed Letter

Gist (paraphrase): Same pattern as 3100 for the priest trainer Priestess Anetta.

- Level: QuestLevel 1, MinLevel 1, MaxLevel not exported [N]. Zone/sort -262 = sort: Priest. Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 16 (Priest), race mask 1 (Human). Flags 0, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Marshal McBride (197) at (-8902.59, -162.61, 82.02) facing 1.988, Northshire Valley, respawn 285 s [V:data]
- Ender: Priestess Anetta (375, Priest Trainer) at (-8853.59, -193.34, 82.12) facing 2.548, Northshire Valley, respawn 270 s [V:data]
- Links: PrevQuestId 7; derived prevQuests [7] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item (given on accept, unless the player already holds the count: CanGiveQuestSourceItemIfNeed): Hallowed Letter (9548) [Quest class 12 / sub 0, common]; it is also the required item, so the quest is COMPLETE immediately after accept [V:src] [V:data]
- Required item: Hallowed Letter (9548) [Quest class 12 / sub 0, common]
  - source: only the quest's own source item (no loot row carries it) [V:data tsv]
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 40 (direct XP, see 1.8). RewMoneyMaxLevel 30 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 586 of `cache/world/northshire.txt`: Details 281 chars ($N x1); Objectives 75 chars (none); OfferRewardText 240 chars (none); RequestItemsText 330 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Class gate: mask 16 = Priest; the Human Warrior never sees it. Source item = required item (complete at accept); the trainer ender is in the Abbey.

### 3104 - Glyphic Letter

Gist (paraphrase): Same pattern as 3100 for the mage trainer Khelden Bremen.

- Level: QuestLevel 1, MinLevel 1, MaxLevel not exported [N]. Zone/sort -161 = sort: Mage. Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 128 (Mage), race mask 1 (Human). Flags 0, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Marshal McBride (197) at (-8902.59, -162.61, 82.02) facing 1.988, Northshire Valley, respawn 285 s [V:data]
- Ender: Khelden Bremen (198, Mage Trainer) at (-8851.57, -188.23, 89.50) facing 2.915, Northshire Valley, respawn 270 s [V:data]
- Links: PrevQuestId 7; derived prevQuests [7] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item (given on accept, unless the player already holds the count: CanGiveQuestSourceItemIfNeed): Glyphic Letter (9571) [Quest class 12 / sub 0, common]; it is also the required item, so the quest is COMPLETE immediately after accept [V:src] [V:data]
- Required item: Glyphic Letter (9571) [Quest class 12 / sub 0, common]
  - source: only the quest's own source item (no loot row carries it) [V:data tsv]
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 40 (direct XP, see 1.8). RewMoneyMaxLevel 30 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 587 of `cache/world/northshire.txt`: Details 270 chars ($N x1); Objectives 76 chars (none); OfferRewardText 414 chars ($B x2, $N x1); RequestItemsText 61 chars ($N x1); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Class gate: mask 128 = Mage; the Human Warrior never sees it. Source item = required item (complete at accept); the trainer ender is in the Abbey.

### 3105 - Tainted Letter

Gist (paraphrase): Same pattern as 3100 for the warlock trainer Drusilla La Salle.

- Level: QuestLevel 1, MinLevel 1, MaxLevel not exported [N]. Zone/sort -61 = sort: Warlock. Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 256 (Warlock), race mask 1 (Human). Flags 0, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Marshal McBride (197) at (-8902.59, -162.61, 82.02) facing 1.988, Northshire Valley, respawn 285 s [V:data]
- Ender: Drusilla La Salle (459, Warlock Trainer) at (-8926.74, -195.59, 80.77) facing 2.793, Northshire Valley, respawn 270 s [V:data]
- Links: PrevQuestId 7; derived prevQuests [7] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item (given on accept, unless the player already holds the count: CanGiveQuestSourceItemIfNeed): Tainted Letter (9576) [Quest class 12 / sub 0, common]; it is also the required item, so the quest is COMPLETE immediately after accept [V:src] [V:data]
- Required item: Tainted Letter (9576) [Quest class 12 / sub 0, common]
  - source: only the quest's own source item (no loot row carries it) [V:data tsv]
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 40 (direct XP, see 1.8). RewMoneyMaxLevel 30 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 588 of `cache/world/northshire.txt`: Details 274 chars ($N x1); Objectives 80 chars (none); OfferRewardText 383 chars ($B x2); RequestItemsText 155 chars ($N x1); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Class gate: mask 256 = Warlock; the Human Warrior never sees it. Source item = required item (complete at accept); the trainer ender is in the Abbey.

### 3903 - Milly Osworth

Gist (paraphrase): Willem sends the dependable recruit to Milly Osworth, who has trouble at her wagon near the stable behind the Abbey.

- Level: QuestLevel 4, MinLevel 2, MaxLevel not exported [N]. Zone/sort 9 = Northshire Valley (area 9). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Deputy Willem (823) at (-8933.54, -136.52, 83.45) facing 1.972, Northshire Valley, respawn 270 s [V:data]
- Ender: Milly Osworth (9296) at (-8850.29, -224.03, 81.78) facing 4.939, Northshire Valley, respawn 270 s [V:data]
- Links: PrevQuestId 0; derived prevQuests [18, 33] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 3904 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Objectives: none (a talk quest: no counters, completable as soon as it is accepted) [V:data, logic V:src]
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 35 (direct XP, see 1.8). RewMoneyMaxLevel 30 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 589 of `cache/world/northshire.txt`: Details 285 chars ($B x2, $N x1, $c x1); Objectives 25 chars (none); OfferRewardText 223 chars ($B x2); RequestItemsText 0 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: No PrevQuestId of its own: its prerequisite comes from NextQuestId 3903 on both 18 and 33 (ObjectMgr::LoadQuests pushes 18 and 33 into 3903.prevQuests). Because those two share ExclusiveGroup -18, BOTH must be rewarded (section 1.5).
- Note: NextQuestInChain 3904: Milly offers 3904 on turn-in.

### 3904 - Milly's Harvest

Gist (paraphrase): Milly's grape harvest sits in buckets in the vineyards to the southeast, where Defias have moved in; she wants 8 crates saved.

- Level: QuestLevel 4, MinLevel 2, MaxLevel not exported [N]. Zone/sort 9 = Northshire Valley (area 9). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Milly Osworth (9296) at (-8850.29, -224.03, 81.78) facing 4.939, Northshire Valley, respawn 270 s [V:data]
- Ender: Milly Osworth (9296) at (-8850.29, -224.03, 81.78) facing 4.939, Northshire Valley, respawn 270 s [V:data]
- Links: PrevQuestId 3903; derived prevQuests [3903] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 3905 (auto-offer); derived prevChainQuests [3903]; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Required item: Milly's Harvest (11119) x8 [Quest class 12 / sub 0, common]
  - source: object Milly's Harvest (161557), loot table 10119: 100% QUEST drop, 40 spawns in the valley
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 180 (direct XP, see 1.8). RewMoneyMaxLevel 120 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 590 of `cache/world/northshire.txt`: Details 493 chars ($B x4); Objectives 71 chars (none); OfferRewardText 226 chars ($B x2, $N x1); RequestItemsText 29 chars ($N x1); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: The 8 items are collected by using crates; kills are not involved. 40 crate spawns, 180 s respawn, consumable (section 5).
- Note: NextQuestInChain 3905: Milly offers 3905 on turn-in.

### 3905 - Grape Manifest

Gist (paraphrase): Milly gives a manifest for Brother Neals, who keeps the food and wine stores in the Abbey bell tower.

- Level: QuestLevel 4, MinLevel 2, MaxLevel not exported [N]. Zone/sort 9 = Northshire Valley (area 9). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 0, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Milly Osworth (9296) at (-8850.29, -224.03, 81.78) facing 4.939, Northshire Valley, respawn 270 s [V:data]
- Ender: Brother Neals (952) at (-8902.13, -181.65, 113.24) facing 0.873, Northshire Valley, respawn 430 s [V:data]
- Links: PrevQuestId 3904; derived prevQuests [3904] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests [3904]; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item (given on accept, unless the player already holds the count: CanGiveQuestSourceItemIfNeed): Grape Manifest (11125) [Quest class 12 / sub 0, common]; it is also the required item, so the quest is COMPLETE immediately after accept [V:src] [V:data]
- Required item: Grape Manifest (11125) [Quest class 12 / sub 0, common]
  - source: only the quest's own source item (no loot row carries it) [V:data tsv]
- Reward choice (pick 1): Wine-stained Cloak (11475) [armor class 4 / sub 1 = Cloth, inv 16 = Back, armor 5, common]; Latched Belt (2690) [armor class 4 / sub 3 = Mail, inv 6 = Waist, armor 38, common]
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 355 (direct XP, see 1.8). RewMoneyMaxLevel 210 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 591 of `cache/world/northshire.txt`: Details 288 chars ($B x2); Objectives 62 chars (none); OfferRewardText 288 chars ($B x4, $n x1); RequestItemsText 70 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Source item 11125 = required item 11125: complete at accept. Brother Neals stands at z 113.24 (bell tower).

### 5261 - Eagan Peltskinner

Gist (paraphrase): A short pointer: Eagan Peltskinner, around the left side of the Abbey, is looking for a wolf hunter.

- Level: QuestLevel 2, MinLevel 1, MaxLevel not exported [N]. Zone/sort 9 = Northshire Valley (area 9). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Deputy Willem (823) at (-8933.54, -136.52, 83.45) facing 1.972, Northshire Valley, respawn 270 s [V:data]
- Ender: Eagan Peltskinner (196) at (-8869.22, -163.24, 80.97) facing 0.960, Northshire Valley, respawn 270 s [V:data]
- Links: PrevQuestId 783; derived prevQuests [783] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 33 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Objectives: none (a talk quest: no counters, completable as soon as it is accepted) [V:data, logic V:src]
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 85 (direct XP, see 1.8). RewMoneyMaxLevel 60 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 592 of `cache/world/northshire.txt`: Details 247 chars ($B x2); Objectives 29 chars (none); OfferRewardText 83 chars (none); RequestItemsText 0 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: No objectives: complete at accept. NextQuestInChain 33: Eagan offers 33 on turn-in.

### 5623 - In Favor of the Light

Gist (paraphrase): Priest-only: Anetta sends the young priest on to Priestess Josetta in Goldshire.

- Level: QuestLevel 4, MinLevel 5, MaxLevel not exported [N]. Zone/sort -262 = sort: Priest. Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 16 (Priest), race mask 1 (Human). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: Priestess Anetta (375, Priest Trainer) at (-8853.59, -193.34, 82.12) facing 2.548, Northshire Valley, respawn 270 s [V:data]
- Ender: Priestess Josetta (377, Priest Trainer) - NO spawn in the manifest (outside the valley) [V:data]
- Links: PrevQuestId 0; derived prevQuests [] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 5624 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Objectives: none (a talk quest: no counters, completable as soon as it is accepted) [V:data, logic V:src]
- Item rewards: none
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 90 (direct XP, see 1.8). RewMoneyMaxLevel 60 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 593 of `cache/world/northshire.txt`: Details 348 chars ($gpriest:priestess; x1); Objectives 44 chars (none); OfferRewardText 102 chars ($N x1); RequestItemsText 0 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: Priest-only, MinLevel 5, ender Priestess Josetta (377, Goldshire, not in the valley). NextQuestInChain 5624 "Garments of the Light" is outside the 28.

### 5805 - Welcome!

Gist (paraphrase): Collector's Edition thank-you: hand the Northshire Gift Voucher to Merissa Stilwell for a companion pet.

- Level: QuestLevel 1, MinLevel 1, MaxLevel not exported [N]. Zone/sort 24 = Northshire Abbey (area 24). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 0. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 0, SpecialFlags 0 (not repeatable). [V:data]
- Giver: **none** (no creature_questrelation / gameobject_questrelation row) [V:data]
- Ender: Merissa Stilwell (11940) at (-8908.31, -108.52, 81.93) facing 4.136, Northshire Valley, respawn 270 s [V:data]
- Links: PrevQuestId 0; derived prevQuests [] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item (given on accept, unless the player already holds the count: CanGiveQuestSourceItemIfNeed): Northshire Gift Voucher (14646) [Quest class 12 / sub 0, common]; it is also the required item, so the quest is COMPLETE immediately after accept [V:src] [V:data]
- Required item: Northshire Gift Voucher (14646) [Quest class 12 / sub 0, common]
  - source: only the quest's own source item (no loot row carries it) [V:data tsv]
- Reward choice (pick 1): Diablo Stone (13584) [Miscellaneous class 15 / sub 0, common]; Panda Collar (13583) [Miscellaneous class 15 / sub 0, common]; Zergling Leash (13582) [Miscellaneous class 15 / sub 0, common]
- Money: 0g 0s 0c (0 copper) (RewOrReqMoney 0; positive = reward). XP: RewXP 0 (direct XP, see 1.8). RewMoneyMaxLevel 0 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 594 of `cache/world/northshire.txt`: Details 344 chars (LF x1, $B x4); Objectives 54 chars (none); OfferRewardText 115 chars ($N x1); RequestItemsText 135 chars ($B x2); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: NO giver row. Details in section 2.2.

### 7961 - Waskily Wabbits!

Gist (paraphrase): Designer-island test quest (the text names Jon LeCraft and Designer Island): kill rabbits and polymorph clones.

- Level: QuestLevel 1, MinLevel 1, MaxLevel not exported [N]. Zone/sort 9 = Northshire Valley (area 9). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 9. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 10 = PARTY_ACCEPT+SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: **none** (no creature_questrelation / gameobject_questrelation row) [V:data]
- Ender: **none** (no involved-relation row) [V:data]
- Links: PrevQuestId 0; derived prevQuests [] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Required kill: 60 x Polymorph Clone (16479; template only in the TSV export, level 10-10, type 8; not in the manifest); no spawn in the valley
- Required kill: 60 x The Good Rabbit (14886; template only in the TSV export, level 5-5, type 8; not in the manifest); no spawn in the valley
- Fixed rewards: Rabbit's Foot (3300) x5 [Miscellaneous class 15 / sub 0, poor]; Rabbit Crate (Snowshoe) (8497) [Miscellaneous class 15 / sub 0, common]
- Money: 0g 0s 4c (4 copper) (RewOrReqMoney 4; positive = reward). XP: RewXP 20 (direct XP, see 1.8). RewMoneyMaxLevel 0 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 0. [V:data]
- Texts [V:data], line 596 of `cache/world/northshire.txt`: Details 104 chars ($b x2, $n x1); Objectives 73 chars (none); OfferRewardText 0 chars (none); RequestItemsText 0 chars (none); EndText 25 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: NO giver and NO ender row. Flags 10 = party-accept + sharable. Kill objectives name 16479 "Polymorph Clone" and 14886 "The Good Rabbit", neither spawned in the valley. Details in section 2.2.

### 7962 - Wabbit Pelts

Gist (paraphrase): Designer-island test quest: bring 3 LeCrafty Rabbit Pelts to Jon LeCraft.

- Level: QuestLevel 1, MinLevel 1, MaxLevel not exported [N]. Zone/sort 9 = Northshire Valley (area 9). Type 0, Method 2 (2 = deliver, i.e. NOT auto-complete), SuggestedPlayers 0, RequiredSkill 0, LimitTime 0. `patch` row 4. [V:data]
- Class mask 0 (any), race mask 0 (any). Flags 8 = SHARABLE, SpecialFlags 0 (not repeatable). [V:data]
- Giver: **none** (no creature_questrelation / gameobject_questrelation row) [V:data]
- Ender: **none** (no involved-relation row) [V:data]
- Links: PrevQuestId 0; derived prevQuests [] (own PrevQuestId plus the NextQuestId of other quests that point here); NextQuestId 0; ExclusiveGroup 0; NextQuestInChain 0 (auto-offer); derived prevChainQuests []; BreadcrumbForQuestId 0. [V:data, derivation V:src]
- Source item: none [V:data]
- Required item: LeCraft Rabbit Pelt (19482) x3 [Quest class 12 / sub 0, common]
  - source: no loot row, vendor or object in the data carries it [V:data tsv]
- Fixed rewards: 22 Pound Lobster (13913) [Miscellaneous class 15 / sub 0, common]
- Money: 0g 0s 30c (30 copper) (RewOrReqMoney 30; positive = reward). XP: RewXP 100 (direct XP, see 1.8). RewMoneyMaxLevel 60 copper (paid instead of XP at max level; irrelevant here). RewSpell 0, RewSpellCast 6756. [V:data]
- Texts [V:data], line 597 of `cache/world/northshire.txt`: Details 87 chars ($b x2); Objectives 45 chars (none); OfferRewardText 0 chars (none); RequestItemsText 0 chars (none); EndText 0 chars (none); ObjectiveText1-4 lengths [0, 0, 0, 0] (all empty: the client builds "<name> slain: n/m" itself).
- Note: NO giver and NO ender row; requires item 19482 which no loot table or vendor carries. RewSpellCast 6756. Details in section 2.2.


### 2.2 Quests with no NPC giver: 5805, 7961, 7962

- **Whole-database check [V:data tsv]:** none of the three appears in `creature_questrelation` (3 895 rows), `gameobject_questrelation` (262 rows) or `gameobject_involvedrelation` (209 rows); 5805 appears once in `creature_involvedrelation` (4 060 rows, Merissa Stilwell 11940); 7961 and 7962 appear nowhere; no `game_event_quest` row names them; no vendor, creature loot, object loot or reference loot row carries the voucher 14646. Its siblings 14647-14651 (Coldridge Valley, Shadowglen, Valley of Trials, Camp Narache, Deathknell gift vouchers) exist as items too.
- **5805 Welcome!** [V:data] has **no** row in `creature_questrelation` or `gameobject_questrelation`; only an ender (Merissa Stilwell, 11940, at (-8908.31, -108.52, 81.93) in the valley). Source item = required item = 14646 "Northshire Gift Voucher" (class 12 quest item, not unique, allowed for all classes and races, no `spellid_1`, no vendor sells it, no loot table carries it, not in `game_event_quest`). Reward: choice of three non-combat pet items (13584 Diablo Stone, 13583 Panda Collar, 13582 Zergling Leash; their use spells 17708, 17707, 17709 are not Northshire content) and no XP. The quest text itself says it is the Collector's Edition thank-you [V:data].
  - **How it would be offered:** in vMaNGOS the item is the giver: `Item::HasQuest(quest_id)` is true when the item's `StartQuest` equals the quest, and `HandleQuestgiverQueryQuestOpcode` accepts an item guid as quest object, so right-clicking the voucher opens the quest details (accept window) [V:src Item.h + QuestHandler.cpp]. The DB column that would say so, `item_template.start_quest`, exists in the schema (column list read from the `.frm` of the project's own copy under `cache/mangos/db`) but is **not in the TSV export** [N value; I = the voucher's `start_quest` is 5805]. `RewMailTemplateId` is a *reward* mail of a quest (and is not exported either), so it is not how this quest is offered [V:src]. How a real character received the voucher (in-game mail for Collector's Edition owners) is not in the data [N, I].
  - **Proposal:** do not offer by default (the game has no CE concept); optionally a debug switch that puts the voucher in the player's quest items.
- **7961 Waskily Wabbits!** and **7962 Wabbit Pelts** [V:data]: no start rows, no end rows, no event rows. Their texts name Jon LeCraft and a place called Designer Island: Blizzard test quests. 7961: kill 60 x entry 16479 ("Polymorph Clone", a critter template) and 60 x entry 14886 ("The Good Rabbit") while the objective text says 5 rabbits, flags 10, rewards 5 x Rabbit's Foot and a Rabbit Crate (Snowshoe) (spell 10711), 4 copper, 20 XP; 7962: 3 x "LeCraft Rabbit Pelt" (19482, no loot/vendor source), reward 22 Pound Lobster, 30 copper, 100 XP, RewSpellCast 6756. Neither creature is spawned in the valley [V:data]. They are in the manifest only because their ZoneOrSort is 9 (`select.rs` rule "zone = Northshire Valley"). **Proposal:** exclude orphans (quests with neither a giver nor an item-start) from the playable set; keep them in the manifest as data if wanted.

### 2.3 Quests that need an NPC outside the valley, and what the game should do

All of these NPCs have a creature template in the manifest but **no spawn** (the valley test is by area, the importer kept the templates so the relations point somewhere) [V:data].

| quest | role of the outside NPC | NPC | notes |
|---|---|---|---|
| 54 Report to Goldshire | ender | Marshal Dughan (240), Goldshire, level 25 | last quest of the McBride line; reward Pikeman Shield (fixed) + 225 XP; texts for Dughan are in the manifest |
| 2158 Rest and Relaxation | ender | Innkeeper Farley (295, innkeeper+vendor), Lion's Pride Inn, Goldshire | giver Falkhaan Isenstrider (6774) stands at the valley's south edge (-9044.56, -45.98, 88.42); 110 XP; choice of 5 Small Pumpkin / 5 Refreshing Spring Water |
| 5623 In Favor of the Light (Priest only, not the Warrior's) | ender | Priestess Josetta (377), Goldshire | chain 5624 outside the 28 |
| 344 Brother Paxton | giver | Milton Sheaf (1440), Stormwind | needs 343 (giver Brother Kristoff 1444, Stormwind); MinLevel 20 |
| 345 Ink Supplies | ender | Foreman Oslow (341), Lakeshire | MinLevel 20; the next quest 347 is also given by Oslow |
| 347 Rethban Ore | giver | Foreman Oslow (341) | needs 5 Rethban Ore, which nothing in the valley carries |
| 346 Return to Kristoff | ender | Brother Kristoff (1444), Stormwind Cathedral Square | CompleteScript 346 [N] |

What the game should do (my recommendation; the user decides, open question 3):
1. **Stormwind chain 343-347: leave out of G3.** Their level-20 gate and their givers/enders make them unreachable; Brother Paxton (valley NPC) would show no mark for them for a character who cannot have 344 [V:src logic: 345 needs 344 rewarded, 346 needs 347 rewarded].
2. **54 and 2158: accept normally, then park.** The quest enters the log (54 already COMPLETE because of its source item), the log shows the right objective text, and nothing can turn it in; the engine logs one line "ender X is outside the valley" (data-correctness rule 4 of the design spec). Rewards and XP of these two are stored but never paid until Goldshire exists. This keeps the valley data exact and costs nothing.
3. **Optional later:** (a) stand-in NPC at the valley's southern exit for Dughan and Farley, or (b) an auto turn-in trigger at the exit. Either changes WoW fidelity; neither should be in G3 without the user's yes.

---

## 3. Text format

### 3.1 Placeholder forms in the 28 quests (counted by script over Details, Objectives, OfferRewardText, RequestItemsText, EndText and the four ObjectiveText fields) [V:data]

| form | total | where | notes |
|---|---|---|---|
| `$B` | 92 | Details 48, OfferReward 36, RequestItems 8 | paragraph break; `$B$B` pairs separate paragraphs (blank line) |
| `$b` | 14 | Details 12, OfferReward 2 | lower-case twin; only in 1598 (x10), 7961 (x2), 7962 (x2) |
| `$N` | 27 | Details 12, OfferReward 8, RequestItems 7 | player name |
| `$n` | 3 | Details 2, OfferReward 1 | lower-case twin; only in 1598, 3905, 7961 |
| `$c` | 4 | Details 3, OfferReward 1 | class word in running text: quests 54, 783, 3101, 3903 |
| `$C` | 1 | OfferReward 1 | quest 3100 |
| `$gbrother:sister;` | 1 | RequestItems of 3101 (Paladin letter) | gender switch |
| `$gman:woman;` | 1 | OfferReward of 3102 (Rogue letter) | gender switch |
| `$gpriest:priestess;` | 1 | Details of 5623 (Priest) | gender switch |
| literal line feed (LF) | 1 | Details of 5805 (trailing) | a real newline character, not a token |

Not present at all [V:data]: `$r $R`, `$p`, `$t $T`, `$a`, `$i`, `$l`, `$G` (upper case) , `|cAARRGGBB ... |r` colour codes, `|H..|h` links, `<name>`-style tokens (the brief asked for these; count 0), non-ASCII characters (all 28 texts are plain ASCII), tabs. `ObjectiveText1-4` are empty for all 28, and the `Objectives` field never contains a placeholder. Double spaces after full stops are common and part of the text; `RequestItemsText` of 3904 and Details of 5805 end with whitespace.

### 3.2 How the client expands them for a Human Warrior named by the player (call her/him NAME)

The server does not expand them: vMaNGOS sends the template text as stored (its own fallback greeting is sent with the `$N` token still in it, and `GossipDef.cpp` has no replacement code) [V:src], so the client does it [I]. Documented tags: `$B` line break, `$N` name, `$R` race, `$C` class, `$G male:female;` (AzerothCore wiki, quest_template) [V:doc]; the lower-case variants and their case behaviour are the established convention of the client [I].

| in the text | the 1.12 client shows | notes |
|---|---|---|
| `$N`, `$n` | NAME as typed at character creation | no case change [I] |
| `$C` | `Warrior` | capitalised [I] |
| `$c` | `warrior` | lower case, used in running text (for example after "young" in 783 and after "a" in 54) [I] |
| `$R` / `$r` | `Human` / `human` | not used by the 28 |
| `$B`, `$b` | a line break; two in a row give an empty line | `$b` equal to `$B` [I] |
| `$gA:B;` | A for a male character, B for a female one | `brother`, `man`, `priest` for a male Human Warrior [I]; the game has only a male Human model (`cache/dsr/human_male`) so the choice is [I] male |
| LF character | line break | |

Among the 14 quests the Warrior can take, the ones with a name/class token are: 54 ($N, $c), 783 ($c, $N), 3100 ($N, $C), 3903 ($c, $N), the $N-only quests 6, 7, 15, 21, 33, 3904 and `$n` in 3905; 18, 2158 and 5261 have only `$B`. None of the 14 contains a gender switch (the three switches are in 3101, 3102 and 5623, other classes). Implementation hint for G3: one function `expand(text, &PlayerIdentity{name, class, race, gender})` handling `$[nN] $[cC] $[rR] $[bB]` and `$[gG]a:b;` with a regexp; unknown `$x` forms are left unchanged and logged once.
Open (question 5): the player has no character creation, so the **name** and **gender** are undefined.

---

## 4. UI flow as the 1.12 client does it (short)

Server side from vMaNGOS [V:src: QuestHandler.cpp, Player.cpp `PrepareQuestMenu`/`SendPreparedQuest`, `WorldSession::GetDialogStatus`, GossipDef.cpp]; client appearance [I] unless noted.

1. **Overhead marks** (`GetDialogStatus`, highest status of two passes wins): *ender pass* (the NPC takes the quest): quest COMPLETE and not rewarded -> "reward" status (yellow "?"); quest INCOMPLETE -> "incomplete" status (grey "?"). *Giver pass* (the NPC starts the quest, status NONE, `CanSeeStartQuest` true): player level below MinLevel -> "unavailable" (grey "!"); otherwise "available" (yellow "!") if the player's level <= QuestLevel + `Quests.LowLevelHideDiff` (default 4), else the "chat" status (no exclamation). The brief's "available low level grey" is this: below MinLevel = grey "!", too-high-level trivial quest = hidden [V:src for the statuses and defaults, I for the art; which of the two the 1.12 client draws as a grey "!" and which as nothing is not certain, [I]]. Exact numeric values of the status enum were not retrievable [N]; the engine can use its own enum.
2. **Greeting / quest list:** interacting runs `HandleQuestgiverHelloOpcode`: NPCs with gossip (McBride) show the gossip window with the quests as entries; quest-only NPCs (flag 2) go straight to the quest: `SendPreparedQuest` with exactly one menu entry opens its dialog directly (details if available; progress if in progress or complete), several entries give a list (title = the NPC's greeting where it has one) [V:src].
3. **Details window** (SMSG_QUESTGIVER_QUEST_DETAILS): title, Details, Objectives, reward preview (choices, fixed items, money, XP), buttons Accept / Decline. Accept -> `AddQuest` (log slot, source item given, counters synced to items already held), then `CompleteQuest` at once if objectives are already met (letters, documents, no-objective quests) [V:src].
4. **Progress window** ("request items", SMSG_QUESTGIVER_REQUEST_ITEMS) when the quest is not yet complete: RequestItemsText plus the required items; the Continue button is enabled only when the quest can be completed. If RequestItemsText is empty, or the quest is complete with no items, the server goes straight to the reward window [V:src `SendQuestGiverRequestItems`].
5. **Completion window** ("offer reward", SMSG_QUESTGIVER_OFFER_REWARD): OfferRewardText, the choice items (pick one, index < 6; a choice is only needed when the quest has a choice list), fixed items, money, XP; "Complete Quest" turns it in: `RewardQuest` takes the required items first, then gives items, money, XP, spells (SMSG_QUESTGIVER_QUEST_COMPLETE shows the XP/money) and immediately opens the next quest of the chain if the same NPC offers it [V:src].
6. **Quest log:** up to 20 quests [I]; entry = title, the `Objectives` line, one line per objective "Kobold Vermin slain: 3/10" (kills, `ObjectiveText` empty so the client uses the creature name) or "Red Burlap Bandana: 4/12" (items), "(Complete)" marker when done, reward preview [I]. Abandon -> `RemoveQuestAtSlot`; it removes the source item and returns the quest to NONE (it can be taken again) [V:src partly, I].
7. **Progress feedback:** SMSG_QUESTUPDATE_ADD_KILL (entry, current, required) and SMSG_QUESTUPDATE_ADD_ITEM make the client print the "name slain: n/m" / "item: n/m" line in the centre-of-screen message frame and chat, and play the quest-progress sound; when the last counter fills it adds an "objective complete" message and sound [I]. The source item (documents, letters, manifest) produces **no** item update line [V:src `ItemAddedQuestCheck` skips the update for the source item].
8. **DS mapping (spec G3):** marks over NPC heads, a dark dialogue plate with the details/progress/completion texts as menu lines, a quest log screen, and a banner toast for "objective changed" / "quest complete"; none of this changes the rules above.

---

## 5. Kill and loot rules

### 5.1 Kill credit (Player::KilledMonster / KilledMonsterCredit, Player.cpp) [V:src]

- A kill calls `KilledMonster(creatureInfo, guid)` -> `KilledMonsterCredit(cInfo->entry, guid)`: **only the creature's own entry counts**; this DB's `creature_template` has no KillCredit columns (column list of the table read from the `.frm` of the project's cache copy) [V:data].
- For each quest-log slot: status must be INCOMPLETE, the quest must carry the KILL_OR_CAST flag, and in a raid group the quest must be raid-allowed (irrelevant). For each objective j: skip empty and **negative ids** (objects) and spell-cast objectives; match `ReqCreatureOrGOId[j] == entry`; add 1 only while below `ReqCreatureOrGOCount[j]` (counter saturates); send `SendQuestUpdateAddCreatureOrGo` (entry, current, required); then `CompleteQuest` if `CanCompleteQuest`. No alive/range check, no tap or grey-level check in these functions.
- Group: `RewardPlayerAndGroupAtEvent` gives each member within reward distance the credit; irrelevant for a single player (the distance value is [N]).
- The 28 quests' kill objectives [V:data]: **7**: 10 x Kobold Vermin (6; 33 spawns, all area 9, box x -8811..-8750, y -258..-93); **15**: 10 x Kobold Worker (257; 43 spawns: 20 in area 9, 23 in Echo Ridge Mine area 34); **21**: 12 x Kobold Laborer (80; 17 spawns, all in Echo Ridge Mine); 7961 only: the two test creatures. Quests 6, 18, 33, 3904 have no kill objective (they use items).

### 5.2 Quest item drops (LootMgr.h / LootMgr.cpp, Player::HasQuestForItem) [V:src]

- `LootStoreItem` stores `chance = abs(ChanceOrQuestChance)` and `needs_quest = (value < 0)`. So **-80 means an 80% drop, but only for a player for whom the item is still needed**; a value of 100 (or -100) always rolls true.
- Per looter, `LootItem::AllowedForPlayer` requires `Player::HasQuestForItem(item)` for quest items: some quest in the log with status INCOMPLETE that lists the item as `ReqItemId` and whose **progress counter is below the required count** (the counter, not the bag content). So the drop stops as soon as the counter is full, and it never drops when the quest is not in the log. (`ReqSourceId` items use the bag count instead; none of the 28 has `ReqSourceId` [V:data].)
- Counts: `mincountOrRef` 1, `maxcount` 1 for every quest row here (one item per drop) [V:data]. Group 0: rows roll independently with the other rows of the same table, so a thug also rolls its normal loot [V:data/src].
- Quest rows in the data [V:data] (loot rows of the valley creatures; the full loot tables are in the manifest `loot` records): Defias Thug (38; 77 spawns) -> Red Burlap Bandana 752 at -80; Timber Wolf (69; 36) and Young Wolf (299; 37) -> Tough Wolf Meat 750 at -80 each; Garrick Padfoot (103; 1 spawn at (-9056.48, -460.90, 72.83), Northshire Vineyards, stands still, respawn 270 s) -> Garrick's Head 182 at -100; Kobold Miner (40) has two quest rows (Large Candle -66, Gold Dust -50) that are not used by any of the 28 (they belong to Elwynn quests outside the valley [I]).
- Expected effort [V:data, arithmetic]: 12 bandanas at 80% needs on average 15 thug kills (77 thugs exist, 31 near the Abbey side and 46 in the Vineyards); 8 meat at 80% needs on average 10 wolf kills (73 wolves).
- `Item` limits [V:data]: 182, 745, 2795, 6785, 9542-9576 (letters) and 11125 are unique (`max_count` 1); 182 additionally has item flag 0x800 (the party-loot flag [I]); 750, 752, 11119 stack to 20.
- Removing/destroying a required item un-completes the quest (`ItemRemovedQuestCheck` -> `IncompleteQuest`); turn-in destroys the required items before any reward is given (`RewardQuest` order: required items, choice item, fixed items, reputation, XP, money, mail, status rewarded, reward spell) [V:src]. Since G3 has no inventory, treat quest items as counters stored in the quest state (the spec already says "items held").

### 5.3 Objects (GameObject.cpp, GameObjectDefines.h, Player::SendLoot, WorldSession::DoLootRelease)

- **Milly's Harvest 161557** [V:data]: type 3 (chest), display 3012, flags 4 (`GO_FLAG_INTERACT_COND`), `data0..5` = [43 lock, 10119 loot table, 0 restock time, 1 consumable, 0, 0]. Chest field order is lockId, lootId, chestRestockTime, consumable, minSuccessOpens, maxSuccessOpens, eventId, linkedTrapId, questId (data8), level (data9), ... [V:src GameObjectDefines.h]; only data0-5 were exported, so `questId` and `level` are [N]. Loot table 10119 has exactly one row: item 11119 "Milly's Harvest" at -100 (always, quest-only) [V:data]. **40 spawns**, all in Northshire Vineyards (area 59), box x -9114.74..-9026.81, y -379.74..-297.31, z about 73.5, respawn 180/180 s, none in a game event [V:data]. Quest 3904 needs 8 of them.
- The crate is consumable: after it is looted completely the chest goes to "just deactivated" and respawns after the spawn's respawn delay (180 s here); a chest that is not consumable and has a restock time would restock instead (not our case) [V:src `GameObject::Update`, `DoLootRelease`].
- A crate **highlights (sparkle) only for a player whose quest needs it** (`ActivateToQuest`: the player has the quest for the GO entry, or the chest's loot has quest loot for the player) [V:src]. Nothing in `GameObject::Use` or `SendLoot` stops a player without the quest from opening it, and an empty chest is deactivated on release [V:src, partial]; for our single player game: **make it usable only while quest 3904 needs the item, and do not consume it otherwise** [design, I].
- **Stolen Books 83763** (Warlock quest 1598 only): type 3, flags 4, data [93 lock, 4165 loot, 0, 0 (not consumable), 0, 0], loot 4165 = item 6785 at -100, one spawn at (-8957.04, -432.75, 64.72), respawn 180 s [V:data].
- No kill objective of the 28 uses an object id; the spell/object credit path (`CastedCreatureOrGO`, negative ids) is unused [V:data]. 175761 "Civil War in the Plaguelands" (type 9 book) and the campfires/benches have no quest role.
- Respawn of the quest-relevant spawns [V:data]: creatures 270 s (guards 430 s; the quest NPCs 270-430 s); crates and books 180 s. The design spec's G2 rule (max(10 min, 4x DB respawn)) is stricter and applies to monsters; for the 40 crates the DB value is irrelevant in practice because 8 of 40 are needed (open question 8).

---

## 6. Dependency graph, order of play, open questions

### 6.1 Graph (`-->` = unlocks; `==>` = unlocks and is auto-offered at the same NPC on turn-in via NextQuestInChain)

    [783 A Threat Within]  Willem -> McBride
      |
      +==> [7 Kobold Camp Cleanup]  McBride -> McBride
      |      |
      |      +--> [15 Investigate Echo Ridge]  McBride -> McBride
      |      |       |
      |      |       +==> [21 Skirmish at Echo Ridge]  McBride -> McBride
      |      |                |
      |      |                +--> [54 Report to Goldshire]  McBride -> Marshal Dughan (OUTSIDE the valley)
      |      |
      |      +--> [3100 Simple Letter]  McBride -> Llane Beshere     (Warrior + Human only;
      |                                                               3101-3105 are the other classes' letters)
      |
      +--> [5261 Eagan Peltskinner]  Willem -> Eagan
      |       |
      |       +==> [33 Wolves Across the Border]  Eagan -> Eagan ------------+
      |                                                                      |   ExclusiveGroup -18
      +--> [18 Brotherhood of Thieves, MinLevel 2]  Willem -> Willem --------+   (each-from-all:
                                                                             |    BOTH must be rewarded)
                       +-----------------------------------------------------+
                       |
                       +--> [3903 Milly Osworth, L2]  Willem -> Milly
                       |       |
                       |       +==> [3904 Milly's Harvest, L2]  Milly -> Milly
                       |               |
                       |               +==> [3905 Grape Manifest, L2]  Milly -> Brother Neals
                       |
                       +--> [6 Bounty on Garrick Padfoot, L2]  Willem -> Willem
                              (6 is behind 33 only because 18 sits in the negative group; vMaNGOS side effect)

    legend:  --> prerequisite (previous quest rewarded)    ==> prerequisite AND auto-offered at the same NPC on turn-in (NextQuestInChain)

    Stand-alone: [2158 Rest and Relaxation]  Falkhaan Isenstrider (6774) -> Innkeeper Farley (OUTSIDE), no prerequisite
    Other classes: 1598 (Warlock), 5623 (Priest, MinLevel 5, ender OUTSIDE), 3101-3105
    Stormwind chain (never reachable): 343 (Kristoff) -> 344 (Milton Sheaf -> Paxton) ==> 345 (Paxton -> Oslow, Lakeshire) ==> 347 (Oslow -> Paxton) ==> 346 (Paxton -> Kristoff)
    Orphans (no giver): 5805 (item 14646 starts it, voucher, ender Merissa Stilwell), 7961, 7962 (test quests)

### 6.2 Recommended order of play for the Human Warrior (dependencies and geography)

1. **783** from Deputy Willem (-8933.5, -136.5) -> turn in at McBride (-8902.6, -162.6); McBride offers **7** at once. (2158 may be taken from Falkhaan at the south edge at any time; it will stay in the log.)
2. **7**: 10 Kobold Vermin in the woods north (x -8811..-8750, y -258..-93) -> McBride (25 copper, 170 XP). Now McBride has **15** and **3100**; take 3100 (complete at once) and hand it to Llane Beshere (-8918.4, -208.4) whenever.
3. **5261** from Willem -> Eagan (-8869.2, -163.2); Eagan offers **33**: 8 Tough Wolf Meat (Timber/Young Wolves around the valley) -> Eagan (shoes or bracers).
4. **15**: 10 Kobold Workers (area north and inside Echo Ridge Mine) -> McBride (40 copper, 250 XP); McBride offers **21**: 12 Kobold Laborers inside the mine -> McBride (belt, boots or gloves). Then **54** can be taken (documents; it stays in the log).
5. **Level 2 is needed from here (XP rules, open question 1).** **18** from Willem: 12 Red Burlap Bandanas from Defias Thugs (the 46 thugs of the Vineyards and the 31 nearer the Abbey) -> Willem (weapon choice). With 18 and 33 done: **6** (Garrick Padfoot's shack at (-9056.5, -460.9); head to Willem) and **3903** (Willem -> Milly at (-8850.3, -224.0)); Milly offers **3904** at once (8 crates in the Vineyards; the thugs of quest 18 roam the same area, so crates and bandanas can be collected on one trip when 3903 is already done) and then **3905** (complete at once; hand it to Brother Neals in the bell tower at z 113.2).
6. Leftovers in the log: 54 and 2158 (no ender in the valley).

### 6.3 Open questions for the user (the data cannot settle them)

1. **Level gate without XP.** G3 comes before G4 (XP). The five MinLevel 2 quests (18, 6, 3903, 3904, 3905) are unreachable while the player stays level 1. Options: (a) grant XP and levels in G3 (needs the XP table, kill XP and the HP/stamina scaling the spec defers); (b) ignore `MinLevel` until G4; (c) start the player at level 2 or a configurable level. Default proposal: (b), with a single constant.
2. **Quest 6 and the negative group.** vMaNGOS makes 6 wait for 33 too (section 1.5). Replicate it as the data says (default), or treat 6 as "only 18"?
3. **Outside enders (54, 2158, and 5623/Stormwind chain).** Park in the log (default), auto-turn-in trigger at the south exit, or stand-in Dughan/Farley NPCs (section 2.3)? And exclude the Stormwind chain 344-347 and the orphans 5805/7961/7962 from G3 (default: yes)?
4. **Class letters.** Only 3100 (Warrior) is playable. The other five letters and their trainers' dialogue stay data only (default). Llane Beshere must exist as a quest-ending NPC for 3100.
5. **Player name and gender** for `$N` and `$g`: no character creation exists. Default proposal: a configurable name (CLI/env or a first-run text) and male.
6. **Unexported columns** (Appendix A): allow a re-export (`tools/export-mangos.ps1` edit plus a run) so MaxLevel, RequiredCondition, reputation gates, mail rewards, emotes, `start_quest`, scripts and gossip texts are known before the engine is written?
7. **Quest scripts**: `StartScript 54`, `CompleteScript 21` and `346` point at `quest_start_scripts` / `quest_end_scripts`; they usually only make an NPC speak or emote. Needed for exactness? (Not exported.)
8. **Respawn of quest objects.** DB says 180 s for crates; the standing order wants long respawns for monsters. Use the DB value for the 40 crates (default) or the G2 formula?
9. **Reward UI before items exist.** G3 hands out reward items (weapons from 18, belts, shoes, the 54 shield) while the item phase is later: store them as "owned items" in a simple list, or only record the choice and pay later? Also the five weapons of quest 18 (dagger, shortsword, hammer, warhammer, quarterstaff) tie into the roadmap's weapon classes.
10. **Quest-giver marks:** the low-level hide diff (default 4) and the high-level hide diff (7) are from `mangosd.conf.dist.in`, not from the repack's own config (not read). Fine to take the defaults?

---

## Appendix A. Columns to add to the export (everything vMaNGOS rules use that the TSV lacks)

Column lists below were read from the `.frm` files of the project's own database copy (`cache/mangos/db/data/mangos`, read-only, no server started) [V:data]. Add to `tools/export-mangos.ps1`:

- `quest_template`: `MaxLevel`, `RequiredCondition`, `RepObjectiveFaction`, `RepObjectiveValue`, `RequiredMinRepFaction`, `RequiredMinRepValue`, `RequiredMaxRepFaction`, `RequiredMaxRepValue`, `RewRepFaction1-5`, `RewRepValue1-5`, `RewMailTemplateId`, `RewMailDelaySecs`, `RewMailMoney`, `DetailsEmote1-4`, `DetailsEmoteDelay1-4`, `IncompleteEmote`, `CompleteEmote`, `OfferRewardEmote1-4`, `OfferRewardEmoteDelay1-4`.
- `item_template`: `start_quest`, `description`, `page_text`, `bonding`, `extra_flags`, `spellid_2..5` with triggers, `max_durability`, `lock_id`.
- `gameobject_template`: `data6..data23`, `script_name`; `gameobject` columns already include state and spawn flags.
- `creature_template`: `gossip_menu_id`, `trainer_spell`, `trainer_id`, `pickpocket_loot_id`, `skinning_loot_id`, `gold_min/max`, `mechanic_immune_mask` (not quest related; for G1).
- New tables: `quest_start_scripts`, `quest_end_scripts`, `npc_text`, `gossip_menu`, `gossip_menu_option`, `broadcast_text` (greetings), `game_event_creature_quest`-style tables if present, `player_xp_for_level` (for G4).

## Appendix B. Method and checks

- Parsing: the manifest and the TSV were read with a small Python 3 script (`py -3 -I`, same escape rules as `tsv.rs`), all 28 quest records decoded by the field order of `manifest.rs`; manifest vs `quest_template.tsv` compared field by field (0 mismatches). The TSV holds 4 433 quest rows (one per entry, highest patch <= 10).
- The vMaNGOS rules of sections 1.1-1.6 were implemented as a script and run over the 28 quests to produce the dependency table, the waves of 1.12 and the 18/33 matrix.
- The valley relation counts: 25 `start` rows, 26 `end` rows, 0 object relations; 3 quests without a start row (5805, 7961, 7962), 2 without an end row (7961, 7962).
- Reading of the project's own database copy (`cache/mangos/db/data/mangos`, not the repack): only the column-name lists of four `.frm` files, plus one look at raw bytes of `item_template.MYD` to find `start_quest` of item 14646; the MyISAM dynamic-row layout could not be decoded reliably, so that value stays [N]. A `mysqld.exe` process was already running on the machine when I looked; I did not start it, did not connect to it and did not touch it.
- Not done (read-only brief): no connection to any MySQL server; `C:\Spel\MaNGOS` not read or written; the WoW and Dark Souls installs untouched; no cargo run; nothing committed.
