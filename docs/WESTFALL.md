# Westfall: NPC:er, uppdrag och animationsarbete

Genererad från projektets vMaNGOS-export för 1.12 och spelarens WoW-klient. Kör wow-souls --audit-westfall med WOW_DATA för att uppdatera rapporten.

1245 spawnplatser, 100 lokala NPC-/fiendetyper (inklusive alternativa entries), 111 mallar inklusive externa uppdragsrelationer, 48 uppdrag och 1191 patrullpunkter. Sammanfogad värld: 3194 platser och 152 uppdrag.

Databasens säsongsevent och NYI-poster ingår inte i basvärlden. Deadmines-instansen är en separat karta och ingår inte i Westfalls utomhuspopulation. Täthet och rare-pooler väljer aktiva platser ur hela importen enligt spelets befintliga regler. Spirit Healer representeras av gravstenen.

Alla importerade uppdrag finns i paketet. Det betyder inte att eskorter, stavmål eller serverskript är färdigimplementerade. Kolumnen Händelsearbete nedan anger vad som behöver kopplas; ingen sådan sekvens räknas som verifierad av denna import. Nya fienders grundläggande strid och nivåskalning är provisoriska.

## Nya animationspaket per kropp

Dessa visas med klientens egna M2-animationer. Nytt paket: idle, gång/löpning, vändning, attacker, träffreaktioner och död. Käke/näbb, klor, armar eller virvel behöver egna träffbanor.

| WoW-kropp | Alla berörda NPC:er | Nya animationer |
|---|---|---|
| Character\Scourge\Male\ScourgeMale.mdx | Malformed Defias Drone (7051), Venture Co. Drone (7067) | Odöd kroppsrigg: hållning/gång/löpning, närstrid, träffreaktioner och död |
| Creature\AirElemental\AirElemental.mdx | Dust Devil (832) | Svävande virvel, förflyttning/vändning, vindstöt, utfall, träffreaktion och upplösning |
| Creature\CarrionBird\CarrionBird.mdx | Fleshripper (1109), Greater Fleshripper (154), Vultros (462), Young Fleshripper (199) | Ving-/svävloop, flykt/start/landning, dykning, näbbhugg, klor, träffreaktion och fall/död |
| Creature\Crab\Crab.mdx | Sand Crawler (830), Sea Crawler (831), Shore Crawler (1216) | Idle, sidogång/löpning, vändning, vänster/höger klonyp, dubbelnyp, rekyl och död |
| Creature\GnollCaster\GnollCaster.mdx | Riverpaw Herbalist (501), Riverpaw Mystic (453), Riverpaw Shaman (1065) | Egen benmappning för modellen, hållning/gång/löpning, vapen- eller stavattacker, reaktioner och död |
| Creature\GnollMelee\GnollMelee.mdx | Riverpaw Brute (124), Riverpaw Gnoll (117), Riverpaw Taskmaster (98) | Egen benmappning för modellen, hållning/gång/löpning, vapen- eller stavattacker, reaktioner och död |
| Creature\GolemHarvest\GolemHarvest.mdx | Foe Reaper 4000 (573), Harvest Golem (36), Harvest Reaper (115), Harvest Watcher (114), Rusty Harvest Golem (480) | Tung idle/gång, vändning, armsvep/skördehugg, laddat dubbelhugg, träffreaktioner och kollaps |

## Samtliga NPC:er, kategoriserade

Fullständiga modellvägar och kit finns i [WESTFALL-NPCS.tsv](WESTFALL-NPCS.tsv). Antalet platser inkluderar alternativa entries; de aktiveras enligt databasens pooler.

### Eskort och händelser

The Defias Traitor (467, 1 platser), Chicken (620, 28 platser), Daphne Stilwell (6182, 1 platser)

### Gravplats

Spirit Healer (6491, 1 platser)

### P1 – ny kroppsrigg

Harvest Golem (36, 28 platser), Riverpaw Taskmaster (98, 58 platser), Harvest Watcher (114, 35 platser), Harvest Reaper (115, 10 platser), Riverpaw Gnoll (117, 33 platser), Riverpaw Brute (124, 26 platser), Greater Fleshripper (154, 13 platser), Young Fleshripper (199, 35 platser), Riverpaw Mystic (453, 45 platser), Vultros (462, 16 platser), Rusty Harvest Golem (480, 16 platser), Riverpaw Herbalist (501, 49 platser), Foe Reaper 4000 (573, 1 platser), Sand Crawler (830, 17 platser), Sea Crawler (831, 12 platser), Dust Devil (832, 16 platser), Riverpaw Shaman (1065, 4 platser), Fleshripper (1109, 44 platser), Shore Crawler (1216, 22 platser), Malformed Defias Drone (7051, 1 platser), Venture Co. Drone (7067, 2 platser)

### P2 – kastare

Defias Renegade Mage (450, 2 platser), Murloc Minor Oracle (456, 50 platser), Murloc Oracle (517, 58 platser), Defias Pillager (589, 84 platser)

### P2 – namngiven fiende

Old Murk-Eye (391, 1 platser), Sergeant Brashclaw (506, 1 platser), Slark (519, 1 platser), Brack (520, 1 platser), Master Digger (1424, 1 platser), Klaven Mortwake (7053, 1 platser)

### Smådjur

Old Blanchy (582, 1 platser), Deer (883, 2 platser), Cow (2442, 2 platser), Prairie Dog (2620, 9 platser), Snake (2914, 15 platser), Mouse (6271, 4 platser)

### Utanför Westfall – uppdragsrelation

Marshal Dughan (240, 0 platser), Guard Thomas (261, 0 platser), Wiley the Black (266, 0 platser), Clerk Daltry (267, 0 platser), Marshal Haggard (294, 0 platser), Master Mathias Shaw (332, 0 platser), Magistrate Solomon (344, 0 platser), Dungar Longdrink (352, 0 platser), Deputy Rainer (963, 0 platser), Osric Strang (1323, 0 platser), Duthorian Rall (6171, 0 platser)

### Vänlig NPC

Farmer Saldean (233, 1 platser), Gryan Stoutmantle (234, 1 platser), Salma Saldean (235, 1 platser), Farmer Furlbrow (237, 1 platser), Verna Furlbrow (238, 1 platser), Grimbooze Thunderbrew (239, 1 platser), Captain Grayson (392, 1 platser), Protector Bialon (487, 1 platser), Protector Weaver (488, 1 platser), Protector Dutfield (489, 1 platser), Protector Gariel (490, 1 platser), Quartermaster Lewis (491, 1 platser), Thor (523, 1 platser), Defias Messenger (550, 1 platser), Scout Riell (820, 1 platser), Captain Danuvin (821, 1 platser), Lumberjack (842, 4 platser), Gina MacGregor (843, 1 platser), Protector Dorana (869, 1 platser), Protector Deni (870, 1 platser), Protector Korelor (874, 1 platser), Protector Leick (876, 1 platser), Scout Galiaan (878, 1 platser), William MacGregor (1668, 1 platser), Defias Profiteer (1669, 1 platser), Mike Miller (1670, 1 platser), Kriggon Talsone (4305, 1 platser), Westfall Woodworker (6670, 8 platser), Agent Kearnen (7024, 1 platser), Innkeeper Heather (8931, 1 platser), Christopher Hewen (8934, 1 platser), Kirk Maxwell (10045, 1 platser)

### Återanvändbar stridsrigg

Defias Smuggler (95, 73 platser), Defias Pathstalker (121, 11 platser), Defias Highwayman (122, 28 platser), Riverpaw Mongrel (123, 67 platser), Riverpaw Overseer (125, 2 platser), Murloc Coastrunner (126, 43 platser), Murloc Tidehunter (127, 37 platser), Goretusk (157, 54 platser), Murloc Warrior (171, 65 platser), Defias Knuckleduster (449, 18 platser), Riverpaw Bandit (452, 40 platser), Young Goretusk (454, 46 platser), Murloc Hunter (458, 72 platser), Defias Footpad (481, 5 platser), Riverpaw Scout (500, 48 platser), Benny Blaanco (502, 1 platser), Defias Trapper (504, 83 platser), Murloc Netter (513, 52 platser), Murloc Raider (515, 28 platser), Great Goretusk (547, 16 platser), Defias Looter (590, 84 platser), Coyote Packleader (833, 22 platser), Coyote (834, 44 platser), Kobold Digger (1236, 7 platser), Riverpaw Miner (1426, 8 platser), Defias Tower Patroller (7052, 2 platser), Defias Tower Sentry (7056, 2 platser)

## Samtliga uppdrag

Givare och avslutare anges med databas-ID (NPC eller objekt). Kedjor över zongränser kan kräva ytterligare områden. Fullständig kedjeanalys finns i [WESTFALL-QUESTS.tsv](WESTFALL-QUESTS.tsv).

| ID | Uppdrag | Nivå | Givare | Avslutare | Händelsearbete |
|---|---|---:|---|---|---|
| 9 | The Killing Fields | 15 | npc:233 | npc:233 | Standardmål – inte slutspeltestat |
| 12 | The People's Militia | 12 | npc:234 | npc:234 | Standardmål – inte slutspeltestat |
| 13 | The People's Militia | 14 | npc:234 | npc:234 | Standardmål – inte slutspeltestat |
| 14 | The People's Militia | 17 | npc:234 | npc:234 | Standardmål – inte slutspeltestat |
| 22 | Goretusk Liver Pie | 12 | npc:235 | npc:235 | Standardmål – inte slutspeltestat |
| 36 | Westfall Stew | 10 | npc:238 | npc:235 | Standardmål – inte slutspeltestat |
| 38 | Westfall Stew | 13 | npc:235 | npc:235 | Standardmål – inte slutspeltestat |
| 48 | Sweet Amber | 44 | npc:239 | npc:239 | Standardmål – inte slutspeltestat |
| 49 | Sweet Amber | 44 | npc:239 | npc:239 | Standardmål – inte slutspeltestat |
| 50 | Sweet Amber | 44 | npc:239 | npc:239 | Standardmål – inte slutspeltestat |
| 51 | Sweet Amber | 44 | npc:239 | npc:239 | Standardmål – inte slutspeltestat |
| 53 | Sweet Amber | 44 | npc:239 | npc:239 | Standardmål – inte slutspeltestat |
| 64 | The Forgotten Heirloom | 12 | npc:237 | npc:237 | Standardmål – inte slutspeltestat |
| 65 | The Defias Brotherhood | 18 | npc:234 | npc:266 | extern avslutare |
| 67 | The Legend of Stalvan | 28 | npc:267 | obj:3643 | serverskript, extern givare/itemstart |
| 68 | The Legend of Stalvan | 28 | obj:3643 | npc:267 | extern avslutare |
| 102 | Patrolling Westfall | 14 | npc:821 | npc:821 | Standardmål – inte slutspeltestat |
| 103 | Keeper of the Flame | 16 | npc:392 | npc:392 | Standardmål – inte slutspeltestat |
| 104 | The Coastal Menace | 20 | npc:392 | npc:392 | Standardmål – inte slutspeltestat |
| 109 | Report to Gryan Stoutmantle | 10 | npc:233, npc:237, npc:240, npc:261, npc:294, npc:963 | npc:234 | Standardmål – inte slutspeltestat |
| 117 | Thunderbrew | 15 | npc:239 | npc:239 | Standardmål – inte slutspeltestat |
| 132 | The Defias Brotherhood | 18 | npc:266 | npc:234 | extern givare/itemstart |
| 135 | The Defias Brotherhood | 18 | npc:234 | npc:332 | Standardmål – inte slutspeltestat |
| 136 | Captain Sander's Hidden Treasure | 16 |  | obj:35 | extern givare/itemstart |
| 138 | Captain Sander's Hidden Treasure | 16 | obj:35 | obj:36 | Standardmål – inte slutspeltestat |
| 139 | Captain Sander's Hidden Treasure | 16 | obj:36 | obj:34 | Standardmål – inte slutspeltestat |
| 140 | Captain Sander's Hidden Treasure | 16 | obj:34 | obj:33 | Standardmål – inte slutspeltestat |
| 141 | The Defias Brotherhood | 18 | npc:332 | npc:234 | Standardmål – inte slutspeltestat |
| 142 | The Defias Brotherhood | 18 | npc:234 | npc:234 | Standardmål – inte slutspeltestat |
| 143 | Messenger to Westfall | 14 | npc:344 | npc:234 | extern givare/itemstart |
| 144 | Messenger to Westfall | 14 | npc:234 | npc:344 | extern avslutare |
| 151 | Poor Old Blanchy | 14 | npc:238 | npc:238 | Standardmål – inte slutspeltestat |
| 152 | The Coast Isn't Clear | 19 | npc:392 | npc:392 | Standardmål – inte slutspeltestat |
| 153 | Red Leather Bandanas | 15 | npc:878 | npc:878 | Standardmål – inte slutspeltestat |
| 155 | The Defias Brotherhood | 18 | npc:467 | npc:234 | serverskript, händelse/eskort |
| 166 | The Defias Brotherhood | 22 | npc:234 | npc:234 | serverskript |
| 184 | Furlbrow's Deed | 9 |  | npc:237 | extern givare/itemstart |
| 214 | Red Silk Bandanas | 17 | npc:820 | npc:820 | Standardmål – inte slutspeltestat |
| 1650 | The Tome of Valor | 23 | npc:6171 | npc:6182 | Standardmål – inte slutspeltestat |
| 1651 | The Tome of Valor | 25 | npc:6182 | npc:6182 | händelse/eskort |
| 1652 | The Tome of Valor | 25 | npc:6182 | npc:6171 | Standardmål – inte slutspeltestat |
| 2359 | Klaven's Tower | 24 | npc:7024 | npc:332 | serverskript |
| 2360 | Mathias and the Defias | 20 | npc:332 | npc:7024 | Standardmål – inte slutspeltestat |
| 3861 | CLUCK! | 1 | npc:620 | npc:620 | Standardmål – inte slutspeltestat |
| 6181 | A Swift Message | 10 | npc:491 | npc:523 | Standardmål – inte slutspeltestat |
| 6261 | Dungar Longdrink | 10 | npc:1323 | npc:352 | Standardmål – inte slutspeltestat |
| 6281 | Continue to Stormwind | 10 | npc:523 | npc:1323 | Standardmål – inte slutspeltestat |
| 6285 | Return to Lewis | 10 | npc:352 | npc:491 | Standardmål – inte slutspeltestat |
