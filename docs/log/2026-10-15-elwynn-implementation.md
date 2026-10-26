# Elwynn implementation — 2026-10-15

User direction: implement in roster order, top to bottom. Plan: `docs/plans/2026-10-14-elwynn-implementation.md`. Research: `docs/log/2026-10-14-elwynn-roster.md`.

## S1: zone foundation accepted

- Importer accepts `--zone elwynn` while preserving Northshire defaults/API. Area-12 descendants are selected by three-dimensional terrain/WMO floor classification. Stormwind's Jasper Fel and Doc Mixilpixil are excluded; Jasperlode/Fargodeep underground spawns and surface terrain are tested separately. These two WMO corrections account for 1,616 terrain-selected base sites becoming 1,614 imported sites; the roster report was corrected accordingly.
- Manifest v4 adds optional creature reaction/display-weight metadata and spawn pools; versions 1–3 still load with their previous appearance/classification fallback. Optional pool and template-waypoint TSV exports keep older export folders usable. GUID waypoint paths take precedence over template paths.
- One alternative creature is chosen per GUID. Hogger and Thuros pools resolve to one active site each before density. Boss death aliases preserve old Hogger kills at GUID 80731 when the chosen Elwynn site is 81029, and preserve new kills when returning to a legacy manifest. Independent entries and pools allowing multiple simultaneous bosses keep independent deaths.
- Client faction reaction distinguishes flagless villagers from unsupported enemies. Neutral enemies retaliate on a hit, including a hit that holds poise, and calm down on disengage/reset without depending on their remaining health.
- Weighted displays reach all creature kinds. A hostile display must have the same target model as its existing kit; unretargeted female Defias bodies are explicitly left out until S3. Legacy manifests retain the existing species display for hostiles.
- Spawns, quests, loot and shops share the manifest choice. `WOWSOULS_WORLD_FILE` remains an override. Verified `cache/world/elwynn.next.txt` was copied to `cache/world/elwynn.txt`; that file now takes precedence over Northshire, which was retained.

Import totals: **1,614 spawn sites, 197 templates, 90 quests, 95 start/88 end relations, 806 items, 195 object types/769 object sites, 2,955 loot rows, 361 vendor rows, 100 equipment rows, 106 patrol spawns/1,889 waypoint points**. Pool selection yields 1,607 runtime sites before density. S1 runtime census: 261 existing foes, 241 critters, 196 friendly sites and 909 unsupported enemy sites; new combat families are still pending at this checkpoint.

Validation:

- Install-enabled workspace suite: dsr-cache 81, dsr-formats 52, wow-souls 862, wow-world 240 + importer 4, wscl 4; all passed. A later rerun counted 863 game tests, but its relative manifest override was unreadable from Cargo's per-crate test directories, so manifest-based quest checks skipped. This was caught after promoting the absolute default: the Northshire parked-quest test correctly failed because Dughan now exists. Northshire report tests now explicitly read the retained Northshire fixture; a separate Elwynn regression checks that quest 54 can be delivered to Dughan. Both are covered by the final S2 suite.
- Workspace clippy passed with warnings denied; scoped formatting passed before the final display regression test (subsequently formatted locally). Final gates are repeated with S2.
- Unattended Northshire, Goldshire, Eastvale and Hogger checks all printed `WOWSOULS_CHECK ok`. Screenshots inspected. A second Goldshire check confirmed 32 live sites, actual database coordinates and correct ground/upstairs/basement floors, including Farley, Dughan, Keryn, Josetta and Cylina.
- Existing missing `inv_misc_emptyflask_01` icon warning occurs while loading the expanded item catalogue; no ECS faults. Hogger killed the unattended level-1 Mage, as expected. His parked animation quality was not accepted or revised in this stage.
- Spec and code-quality reviews found and resolved legacy hostile appearance, pooled boss save identity and neutral retaliation issues. No code blocker remains in S1.

Evidence/logs (gitignored): `cache/elwynn-survey/{import,workspace-tests,elwynn-tests,clippy,build,live-*}.log`, `{northshire,goldshire,eastvale,hogger,goldshire-npcs}.png`.

## S2

### S2a: existing rig families

Seven new database entries reuse the existing target rigs and source clips. The original eight species retain their tuning and patterns.

| Entry | Name | Level | Display / scale | Weapon display | Selected attacks |
|---:|---|---:|---|---:|---|
| 40 | Kobold Miner | 7 | 373 / 1.15 | 7493 pick | small_1, small_2 |
| 475 | Kobold Tunneler | 6 | 139 / 1.0 | 7440 stone hammer | small_1, small_2 |
| 327 | Goldtooth | 8 | 2299 / 1.15 | 7439 metal hammer | small_1, small_2 |
| 60 | Ruklar the Trapper | 8 | 2153 / 1.3 | 7428 axe | big |
| 79 | Narg the Taskmaster | 10 | 774 / 1.3 | 7439 metal hammer | small_1, small_2 |
| 525 | Mangy Wolf | 6 | 903 / 0.7 | — | bite, lunge |
| 1922 | Gray Forest Wolf | 8 | 380 / 0.85 | — | bite, lunge |

Narg is Rare; Goldtooth/Ruklar and the other four are Normal. New forward/reverse entry mappings preserve XP/quest kill credit. The new rows account for 239 raw sites (122 at default density): Miner 53, Tunneler 37, Goldtooth/Ruklar/Narg one each, Mangy 90, Gray Forest 56. The roster now maps 15 of the 45 base combat entries, with unsupported display bodies still explicitly excluded.

Live checks caught two fitting mistakes before acceptance. The old unarmed kobold bands, scaled to 2.35–3.06 yd, opened too far away for the actual short tools. New tool patterns close to 1.15 × display scale before starting, with stop distance 0.9 × scale. A second pass showed that Ruklar's axe has different marker geometry: the quick swing's blade is 2.41–2.63 yd high at its hit window, while its late arc passes to the side. Both quick moves missed. The existing `big` overhead places the blade 0.78–0.83 yd high and visibly connects: two forced-probe attacks, two hits. Ruklar therefore selects this one level-2, single-window move. The other six entries retain their selected level-1 moves. Common still allows at most two single-window moves at levels 1–2; no radius, damage window or tier gate was widened.

The first replacement test position had a map obstruction at (-8888.86, -132) that stopped Tunneler before its attack range. The unchanged code passes at the old courtyard position and the west meadow (-8898,-94). Final controlled fights use the west meadow and actual database GUIDs/displays/equipment, with all other creature/object sites removed only from gitignored test manifests. A level-11 in-memory test save keeps the player alive for complete motion series.

S2a accepted. All seven entries spawn with their actual display/equipment, attack and hit a stationary player. Game-camera and front recordings were inspected as dense motion series; additional grass-off front series expose the paws/feet and contact arcs. No body stretching or unstable silhouette was found. Default-world Fargodeep also passes: Miner GUID 80615 and Tunnelers 80640/80638/80631/80652 use their distinct database floors, including underground patrols. Runtime census is now 500 foe, 241 critter, 196 friendly and 670 unsupported sites before density.

Final gates passed with both installs enabled and no relative manifest override: wow-souls **869**, wow-world **240 + importer 4**, dsr-cache **81**, dsr-formats **52**, wscl **4**. Scoped formatting, workspace clippy with warnings denied, build and `git diff --check` all pass. Evidence: `cache/elwynn-survey/{s2-*,fight-*,front-*,clear-*,live-fargodeep.log,fargodeep.png}`. No commits.

Net/Pierce Armor/Battle Shout and other named effects remain S6 work. The quest book's 51 “playable” quests describe its data-chain/giver classification, not completion of every combat objective or scripted event in the unfinished zone.

### S2b: Prowler accepted

Entry **118**, level 9–10, display **11415** at **0.85**, now uses its own **61-bone DireWolf** target and the black dog's close bite. Initial tuning: 160 HP, chase 5.9 yd/s, stop 1.0 yd, opening band 0–1.5 yd; normal rank and no held equipment. Its 83 database sites retain paths/wander and respawn data (42 selected at default density). The roster now maps **16 of 45** base combat entries. Runtime census before density: **583 foe, 241 critter, 196 friendly, 587 unsupported** sites.

The measured rig is in `bonemap::DIREWOLF_RIG`: common root 19, hind hips 44/45, paws 58/59, front paws 53/54, jaw 35/head 25. The root is tested as the common ancestor of every mapped body bone. Bite `a00_3000`, seven flinches and death `a00_2205` pass the unchanged direction/hip/0.45-yd contact gates. Bite foot correction is 0.153 yd, death 0.112 yd; death ends at 0.448 of its starting body height. All surveyed lunges need 0.605–0.696 yd correction and fail, so they remain excluded. Common has one level-1, single-window attack; its source timing is 0.633–0.867 s of 1.667 s, phys 104, damage 19 before armor. No gate or hit volume was enlarged.

`Kit.reference_dir` separates target-scaled DS reference clips: DireWolf emits `reference direwolf_ds`, while the five original kits retain their character folders. Attack, react, death and parried pairs all use the selected folder. Runtime accepts the optional line with the old chr fallback. Two targets sharing `c3340` can now keep different scales without overwriting one another. All **20** existing Wolf/source clips stayed byte-identical after the full extraction. This is an additive kit/manifest extension; extractor version remains 5, with no retarget-algorithm change. Mage casts remain valid.

Validation:

- Scoped formatting, workspace clippy with warnings denied, full install-enabled workspace tests and game build passed: **wow-souls 871, wow-world 240 + importer 4, dsr-cache 85, dsr-formats 52, wscl 4**. The first full run caught the old test's explicit kit-name allowlist; it now includes DireWolf. Runtime regression covers all four clip kinds with separate references; actual install regression extracts Wolf then DireWolf and verifies the original bytes and different scales. Spec and code-quality reviews found no blockers.
- Normal controlled fights hit a stationary player for 18 after armor. Dense game-camera and grass-off front series expose the bite and paws; the wider observer offset (7,4,-7) keeps the body in frame. Level-1/2 reaction and death loops were reviewed in motion beside the source skeleton. No body stretching or unstable silhouette was found.
- A separate kill/settlement check uses the existing `WOWSOULS_FOOTWORK=0` instrument so the scripted attacker can keep facing the target; normal fight checks retain footwork. Three staff hits kill Prowler, its corpse holds the side-fall pose, corpse loot uses entry 118 (2672), XP uses entry 118 (+71 at rolled level 9), souls +80. Respawn is 1080 seconds and waits until the player is at least 60 yd away. Test saves are isolated; no player save or production AI was changed.
- The default Elwynn manifest passes at (-9698,-1089,42). Prowlers GUID 80878/81182/81184 are placed on their DB ground and follow 6/8/10-node paths. Every run prints `WOWSOULS_CHECK ok`; the previously logged missing flask icon remains.

Evidence (gitignored): `cache/elwynn-survey/{direwolf-*,s2b-*,fight-prowler.*,front-prowler.*,prowler-a00_*,kill-prowler.*,live-prowler-habitat.*,prowler-habitat-series.jpg}`. No commits. Next: S3 female Defias and melee roles, then enemy casts and goblin/summons.

## Follow-up: Northshire aggression regression

The user's live check caught a behavior regression after the Elwynn import. The original client factions mark Vermin 6, Worker 257, Laborer 80, Timber Wolf 69 and Young Wolf 299 neutral. S1 applied the generic retaliation-only component to them, overriding the intended WoW Souls starter fights. The runtime now explicitly keeps these five enemy entries aggressive on sight. Imported faction data stays intact; other neutral enemies retain retaliation behavior and friendly NPCs keep their classification.

The regression test covers the five starter entries, other neutral enemies, friendly creatures and old manifests. Five separate unattended proximity fights preserve the imported neutral metadata, use no player attack input, and all detect/attack/hit the stationary player (`WOWSOULS_CHECK ok`). The corrected binary is built. A final run using the default Elwynn manifest at an actual Northshire Vermin site also notices and hits the idle player, then prints `WOWSOULS_CHECK ok` (`northshire-default-aggro.log/mp4`). Evidence: `cache/elwynn-survey/{northshire-aggro-*,check-northshire-aggro.ps1,fight-aggro-*}`.

## S3 preparation

User direction: continue implementation; the user will review all changes together in one large later playtest. Keep internal tests and motion checks, without per-family approval requests.

HumanFemale is 117 bones (male 119), with common body root 20, pelvis 22, spine 21/chest 23/neck 44/head 63, right hand 96/forearm 78, feet 66/68 and toes 79/80. The thief's two slash attacks, seven flinches and two deaths were numerically fitted. Measured bind sole offset 0.0269 yd is preserved, toe directions aligned, and ground-lock adjusts the actual poses; maximum contact correction 0.05818 yd remains under the unchanged 0.08 yd gate. Kit `thug_female` uses its own target-scaled `thug_female_ds` references. Runtime stores the chosen kit on the spawned Species component while preserving its base identity, stats, equipment and kill-credit entry; attack/reaction/death/defense/audio/clip-view paths all use that selected kit. Legacy caches lacking the new target explicitly keep the unsupported female body out.

S3a female target accepted: full install-enabled tests/clippy/scoped format and game build pass (873 game, 240 world +4 importer, 86 extractor, 52 formats, 4 wscl). Display 5036 spawns with 117 bones and its actual dagger 2704. Both slashes connect in normal fights; game-camera and grass-off front motion series show stable limbs, silhouette and contacts. Level 1/2 reactions and the full side-fall death were reviewed beside the target-scaled source skeleton; a real kill uses entry 38 loot/souls and the normal 1080-second respawn with 60-yard player-clearance. No named melee entries added yet. Manual spec/code review checked target dispatch, the legacy-cache refusal, all clip consumers and unchanged source/fit limits. Evidence: `cache/elwynn-survey/{s3a-*,*thug-female*,thug-female-a00_*}`.

Remaining source findings for the next implementation: Erlan 880 carries item 5289 (2H axe/pick,display 3797); c0000 axe_2h a30_4000/4001 are single-hit level 2 with TAE 0.6333–0.7667s, while greataxe a32 is level 3. Surveyed kick judge 410/420 is level 5 and must not silently bypass Common. Enemycaster c4160 has single-projectile 3003/3004 (release 1.833/1.100 s), while 3005 is an eight-shot barrage. Creature bullet TAE is type 2; the existing player helper only reads type 64. Geomancer 476 + Surena 881 use Fireball 20793; Rogue Wizard 474 uses Frostbolt 13322; all three use Frost Armor 12544. Detailed raw survey/helpers remain under `cache/elwynn-survey/`.

## S3b — six human melee entries

Added the next six entries supported by the verified human targets. All use the thief's two single-hit level-1 slashes (c2520 a00_3003/3004, original TAE windows and params). No Common budget, source flinch level, fit gate or attack radius changed. These HP values are game tuning; display, level, rank and equipment come from the imported database.

| Entry / species | Level / rank | Display / body | Main / off-hand item display | HP |
|---|---|---|---|---|
| 61 / thuros | 11 Rare | 3341 male | 7492 / 7492 | 180 |
| 94 / cutpurse | 5–6 normal | 2361 male / 2362 female, equal weights | 7434 / none | 90 |
| 99 / morgaine | 10 Rare | 3320 female | 6469 / none | 160 |
| 473 / morgan | 10 normal | 2074 male | 7492 / none | 140 |
| 6093 / dead_tooth_jack | 11 normal | 4947 male | 7428 / none | 160 |
| 13159 / james_clark | 12 normal | 13091 male | 7483 / none | 180 |

Morgaine explicitly selects `thug_female`; Cutpurse resolves the target from its actual display. The missing-female-cache refusal also covers an always-female default. A new optional species off-hand weapon uses the existing left-palm attachment/grip. Thuros's second sword stays in that hand through attacks, reactions and death; his two attacks hit with the main blade only, with no invented extra damage window. Existing species keep no off-hand weapon and retain their old tuning.

The six world mappings preserve real entry kill credit, imported displays, movement, pools, reaction and respawn. Thuros remains one active rare across four pool sites. James remains neutral until provoked; the five Northshire starter aggression exceptions stay intact. S6 retains Backstab/Gouge/Poison and the named encounter mechanics; these six rows do not claim those effects are complete. Erlan, Bandit and Dockworker remain pending their actual 2H/kick roles rather than inheriting an unsuitable replacement.

Validation: scoped format, workspace clippy with warnings denied, install-enabled workspace tests (873 game, 240 world +4 importer, 86 extractor, 52 formats, 4 wscl), game build and diff whitespace check pass. Expanded tests verify the six DB levels/displays/main/off-hand gear/ranks, one-active Thuros pool, James reaction, kill-credit mapping and Common attack budget. Independent spec and code-quality reviews found no gaps.

Live checks: all five aggressive entries attack/hit an idle player, with separate male/female Cutpurse runs. James stays idle nearby for eight seconds and then attacks/hits after exactly one player attack. Dense game-camera and front series cover both slash shapes, body proportions, feet and held weapons; a separate reviewer accepted Cutpurse/Morgaine motion. Actual kills of Thuros/Morgaine give +100/+86 XP, +100 souls each, correct entry loot and first-kill best items 2073/3283. Respawn is 36000/28800 seconds from the selected DB maxima x4, with 60-yard player clearance; both deaths settle in front recordings. A first observer take did not lock on, so its incomplete kill was replaced by a fixture with the enemy inside the camera's lock cone and an explicit death assertion.

Default Elwynn habitat check also passes with the normal density: Cutpurse GUID 79887 wears female 2362 and walks its two-node DB path; nearby male/female stand/wander sites retain their rules. Total resolved runtime sites remain 1607: Foe 618, Critter 241, Friendly 196, Unmapped 552 (35 additional foe sites, 22 mapped species).

Evidence (gitignored): `cache/elwynn-survey/{s3b-*,check-defias*,check-james.ps1,fight-{thuros,cutpurse,cutpurse-female,morgaine,morgan,dead_tooth_jack,james_clark}*,front-*,kill-{thuros,morgaine}*,death-front-{thuros,morgaine}*,idle-james_clark.*,live-cutpurse-habitat.*}`. No commits. Next: Erlan's 2H source adapter, real Bandit/Dockworker kicks, then casts and goblin/summons.

## S3c — Erlan's two-handed axe and Snap Kick roles

Three new entries use separate additive kits; their proven c2520 reactions/deaths remain, while attacks come from actual c0000 player rows. Old NPC attack parameters are not substituted for the new moves.

| Entry / species | Level / body | Equipment | HP / attacks |
|---|---|---|---|
| 880 / erlan | 8, male display 3322 | item 5289, display 3797, 2H axe/pick | 150; axe_1 / axe_2 |
| 116 / bandit | 8–9, male 2357 / female 2358 | item 1896, display 7487, short sword | 130; sword / kick |
| 6927 / dockworker | 8–9, male 2357 / female 2358 | item 1896, display 7487, short sword | 110; sword / kick |

HP and pattern weights are game tuning. Both Bandit bodies retain their equal database display weights; Dockworker's imported health multiplier informs HP rather than shrinking the model. World mappings retain actual entry kill credit, equipment, rank, movement and respawn. All three are hostile; Northshire's five starter exceptions and James's neutral retaliation are unchanged.

**Source selection.** Erlan uses Battle Axe row 701000, variation 3000: `a30_4000/4001`, judge 200/210. Both are 2.167 s, single-hit level 2, original window 0.633–0.767 s, phys 114 (21 game damage before armor). Bandit's sword is row 201000, variation 2300: `a23_3000`, judge 0, level 1, 0.533–0.667 s of 2.5 s (14 game damage). These kits never broaden Common's two-role/single-window/level-1–2 limits.

The actual right-foot kick is `a23_3100`, TAE 307 PCBehavior kind 8, judge 410; its original window is 0.433–0.567 s of 1.667 s. The existing kind-1 reader misses this event, so a narrow reader was added only for the new PC monster extraction. Curved-sword alternatives keep the foot low and were excluded. The source kick is level-5 guard break with zero physical multiplier. An explicit `SnapKick` adaptation, restricted to this exact source row, records both source and game response: phys 44 (8 HP), poise 10, level 1, knock 0.45, blockable, guard budget 20. Source level 5/multiplier 0/guard budget 70 remain in provenance. This small-flinch Souls response is a deliberate adaptation of DB ability 8646, not completion of its original stun effect; further named effects remain S6.

**Fitting/runtime.** New `human_axe_2h`, `human_bandit` and `human_bandit_female` kits have separate target-scaled reference folders. HumanFemale's measured 117-bone PC map uses the same body contract as its accepted thief target. Maximum foot correction: axe 0.040 yd, male sword/kick 0.0392, female 0.0627; all pass the unchanged 0.08-yd limit. Per-attack optional metadata carries source scale, authored full weapon rotation and actual foot/toe reach. Weapon orientation follows the sampled source rotation through fades, preserving blade twist; the kick sweeps right foot→toe (male 70→81, female 68→80), never the held sword. Its foot segment cannot trigger a weapon parry, while normal blocking still applies. Legacy caches keep the old scale/reference/grip path; missing new kits are refused explicitly. All **291 old WSCL files are byte-identical** after full extraction.

PC block stamina comes from actual weapon base stamina × attack correction + attack stamina, rather than interpreting the PC row's zero `guardAtkRate` as free blocking. Source budgets are sword 25, axe 60, kick 70; adapted kick 20. The NPC path retains its existing interpretation. Live checks spend 40.8 stamina per axe block and 13.6 per kick with the test guard's stability; repeated axe hits exhaust stamina normally, and a single kick does not force a guard break.

Validation: scoped formatting, workspace Clippy with warnings denied, full install-enabled workspace tests and game build pass: **875 game, 240 world +4 importer, 88 extractor, 52 formats, 4 wscl**. New regressions cover mixed source scales, full grip rotation, inherited toe endpoints, malformed/dangling metadata, source/adapted guard budgets and the new DB mappings/Common budgets. An initial test caught an over-strict runtime reach check: the toe inherits its foot transform without a direct animation track; both indices remain bounded and the start foot must be driven. No animation fitting limit changed. Independent spec and code-quality reviews passed.

S3c accepted after live verification. All three entries attack and hit an idle player, including separate male/female Bandit and Dockworker fights. Forced close-range kicks connect with the right foot on all four bodies. Dense game-camera and grass-free front series cover both Erlan axe shapes and the kicks; an independent reviewer checked female Bandit and both Dockworkers. No body stretch, unstable support foot, detached weapon or grip snap was found. Real kills of Erlan, female Bandit and Dockworker award entry-specific loot and XP (+57/+71/+57 at rolled levels 8/9/8), +100 souls, and retain 1080-second respawn with 60-yard clearance. PC attack→NPC flinch/death transitions pass; extra front deaths show Erlan/female Bandit falling and holding the corpse pose. Kill fixtures use the existing footwork-off instrument; normal fights retain footwork. All test saves are isolated.

Default-world checks use normal density and no display/manifest override. At Brackwell, Erlan GUID 80855 stands on its DB floor (39.86 vs 39.87), with male/female Bandits wandering their 5-yard radii. At Jerod's Landing, Dockworker GUID 80732 stands and female GUID 84587 follows its 32-node DB path on the imported floor (21.84 vs 21.85). The 61 new supported sites (Erlan 1, Bandit 57, Dockworker 3) bring the resolved census to **1607: Foe 679, Critter 241, Friendly 196, Unmapped 491**, across **25 mapped species**. Every run prints `WOWSOULS_CHECK ok`; scoped formatting and diff whitespace check pass. Existing missing flask-icon warning remains unrelated.

Evidence (gitignored): `cache/elwynn-survey/{s3c-*,kick-retarget.*,kick-foot.txt,check-melee-*.ps1,fight-{erlan,bandit*,dockworker*,kick-*}*,front-{erlan-axe_*,bandit*,dockworker*}*,block-{erlan,kick-bandit}*,kill-{erlan,bandit-female,dockworker}*,death-front-{erlan,bandit-female}*,live-{brackwell,jerod}-s3c*}`. No commits. Next: Geomancer/Rogue Wizard/Surena casts and Frost Armor, then goblin/summons.
