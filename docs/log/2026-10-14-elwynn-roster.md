# Elwynn Forest: fiender, NPC:er och DSR-animationskällor

Kartläggning 2026-10-14. Underlag för nästa implementation; inga nya gameplay-fiender eller animationskit har kopplats in i detta steg. Inget committat.

## Slutsats

**45 stridstyper i den vanliga zonens spawndata, 128 övriga NPC-/djurtyper och 33 ytterligare typer som bara har eventspawns. Två ytterligare fiender skapas av lokala scripts: Defias Ambusher och Defias Bodyguard.** Northshire ingår. Mappningsstatus i tabellerna avser 14 oktober; implementationens aktuella status finns i `2026-10-15-elwynn-implementation.md`.

Kobolder, vanliga vargar och flera mänskliga banditer kan dela befintliga attackkällor. Störst nytt arbete: boar, två spider-riggar, bear, murloc, enemy-casts och HumanFemale/GoblinMale. Prowler använder DireWolf, Riverpaw Runt använder Gnoll; de är inte samma modellfiler som Wolf respektive GnollMelee.

## Metod och avgränsning

- Läste slim HANDOVER och aktuella `world.rs`, `species.rs`, extractor-kit och bonemaps.
- vMaNGOS-exportens map 0, patch 10 (1.12); template = senaste patch ≤ 10. Befintliga TSV:er är från 2026-10-09. Kompletterande AI, spell lists och pooldata lästes 2026-10-14 ur en databasCOPY. Originalservern startades inte.
- Klientens ADT/AreaTable avgör område 12 och dess underområden. Alla 22 Elwynn-områden i undersökningen ryms med marginal innanför lästa terränggränser. Roster omfattar grund-entry och alternativa spawnslots; varje GUID kan välja en variant.
- Terrängurvalet gav 1 616 vanliga spawn-GUID:er och 98 event-GUID:er, 208 templates. WMO-gränskontrollen utesluter Jasper Fel (1325) och Doc Mixilpixil (7207), som hör till Stormwind trots terrängens Elwynn-klassning. Den senare hittades vid implementationen 15 oktober. Kvar: **1 614 vanliga spawnplatser och 206 kandidat-templates** inklusive events. Dessa är kandidater i DB, inte samtidiga aktiva fiender eller spelets 0.5-densitet.
- Exakta inomhusunderområden behöver WMO-upplösning vid zonimporten: gruvspawns kan få ytområdet ovanför grottan. Därför betyder exempelvis Mine Spider märkt Northshire i råterrängen inte att den ska flyttas till Echo Ridge.
- Klientens CreatureDisplayInfo/Extra/ModelData verifierade samtliga inventerade displays. FactionTemplate jämfördes med människans faction. Neutral = angripbar/reagerar på angrepp, inte automatiskt aggro.
- 16 DSR-karaktärer lästes lokalt med befintlig read-only `survey`: skelett, attackfönster, dmgLevel, root motion och projectile events. Inga nya retargets har visuellt godkänts. Vanliga DS-fiender saknar oftast engelskt namn i FMG; namnidentiteter korsrefererades med Souls Modding-listan.
- 135 questkopplingar hittades som inventeringsunderlag, inklusive kedjor till andra zoner. Det är inte ett beslut att importera 135 quests till Elwynn. Questkopplade NPC:er utanför zonen finns separat i `quest-connections.tsv`.

## Fiendefamiljer och nya attackkit

| Familj | Antal entry-typer | DSR-källa / alternativ | Vad behövs |
|---|---:|---|---|
| Kobold | 8 | c2060 Infested Ghoul | Återanvänd kobold-kit; named/rare får egen pattern och WoW-effekter. |
| Kobold caster | 1 | c4160 Bloathead Sorcerer | Nytt cast/staff-kit och källbonemap till befintlig koboldrigg. |
| Wolf | 5 | c3340 Undead Attack Dog | Återanvänd wolf-kit för Wolf.m2; Prowler kräver DireWolf-retarget. |
| Human melee | 11 | c2520 Undead Assassin; c2550 Undead Soldier; c2400 Painting Guardian | Dela lätta attacker. Female-rigg och 2H-yxvarianten behöver egna retargets; rares egna patterns. |
| Human caster | 2 | c2650 Necromancer; alternativ befintliga c0000 Mage-casts | Nytt enemy-caster-kit, male/female-retarget, Fireball/Frostbolt/Frost Armor. |
| Goblin | 1 | c2500 Hollow / c2520 Undead Assassin | Ny GoblinMale-retarget; obeväpnad närstrid först; summon Bodyguard. |
| Gnoll | 5 | c2550 Undead Soldier; c2810 Infested Barbarian; Hogger c2250 Taurus Demon | Nytt vanligt gnoll-kit. Kontrollera Gnoll och GnollMelee separat. Hogger-kit byggt men parkerat. |
| Murloc | 4 | c2550 Undead Soldier; c2520 Undead Assassin för dolk | Ny Murloc-rigg; dela 1H-hugg/slag med vapenspecifik orientering och Lurker-dolk. |
| Boar | 4 | c3460 Armored Tusk; c1200 Rat som enklare alternativ | Ny Boar-rigg. DS-boar kräver uttrycklig balansanpassning; inte godkänd av Common-gaten som den är. |
| Spider | 3 | c3400 Crag-Spider | Två nya riggar: Tarantula och MineSpider. Retarget osäker; WoW AttackUnarmed finns som reserv. |
| Bear | 1 | c5360 Great Feline som svag kandidat; c3340/c1200 enbart bettreferens | Ingen direkt DSR-björn. Eget björn-kit med WoW-attack som första säkra bas; DS-katt kräver ny rigg och balansanpassning. |

## Hela vanliga stridsrosterlistan

”Mappad” nedan betyder att `world::foe_of` redan känner entryn. För nya typer anger familjetabellen animationsarbete. Boss/rare behåller egna patterns och statistik utan att alla måste få en unik animation. Neutraliteten i startområdet ska bevaras.

| Entry | Namn | Level | WoW-rank | Familj | Spawnplatser | Reaktion | WoW-förmågor | Spelmappning | Områden |
|---:|---|---:|---|---|---:|---|---|---|---|
| 6 | Kobold Vermin | 1–2 | Normal | Kobold | 33 | Neutral | — | Mappad | 9:Northshire Valley |
| 40 | Kobold Miner | 6–7 | Normal | Kobold | 53 | Hostile | Pierce Armor | Saknas | 9:Northshire Valley;12:Elwynn Forest;18:Crystal Lake;54:Jasperlode Mine;57:Fargodeep Mine |
| 60 | Ruklar the Trapper | 8 | Normal | Kobold | 1 | Hostile | Net, Pierce Armor | Saknas | 54:Jasperlode Mine |
| 79 | Narg the Taskmaster | 10 | Rare | Kobold | 1 | Hostile | Battle Shout | Saknas | 12:Elwynn Forest |
| 80 | Kobold Laborer | 3–4 | Normal | Kobold | 25 | Neutral | — | Mappad | 12:Elwynn Forest;34:Echo Ridge Mine |
| 257 | Kobold Worker | 3 | Normal | Kobold | 43 | Neutral | — | Mappad | 9:Northshire Valley;34:Echo Ridge Mine |
| 327 | Goldtooth | 8 | Normal | Kobold | 1 | Hostile | — | Saknas | 12:Elwynn Forest |
| 475 | Kobold Tunneler | 5–6 | Normal | Kobold | 37 | Hostile | — | Saknas | 12:Elwynn Forest;57:Fargodeep Mine |
| 476 | Kobold Geomancer | 7–8 | Normal | Kobold caster | 8 | Hostile | Fireball, Frost Armor | Saknas | 54:Jasperlode Mine |
| 69 | Timber Wolf | 2 | Normal | Wolf | 36 | Neutral | — | Mappad | 9:Northshire Valley;34:Echo Ridge Mine |
| 118 | Prowler | 9–10 | Normal | Wolf | 83 | Hostile | — | Saknas | 12:Elwynn Forest;86:Stone Cairn Lake;88:Eastvale Logging Camp;798:Ridgepoint Tower |
| 299 | Young Wolf | 1 | Normal | Wolf | 45 | Neutral | — | Mappad | 9:Northshire Valley;12:Elwynn Forest;87:Goldshire |
| 525 | Mangy Wolf | 5–6 | Normal | Wolf | 90 | Hostile | — | Saknas | 12:Elwynn Forest;18:Crystal Lake;60:Forest's Edge;87:Goldshire;89:Mirror Lake Orchard;92:Mirror Lake;120:Westbrook Garrison |
| 1922 | Gray Forest Wolf | 7–8 | Normal | Wolf | 56 | Hostile | — | Saknas | 12:Elwynn Forest;18:Crystal Lake;54:Jasperlode Mine;86:Stone Cairn Lake;91:Tower of Azora |
| 38 | Defias Thug | 3–4 | Normal | Human melee | 77 | Hostile | — | Mappad | 9:Northshire Valley;59:Northshire Vineyards |
| 61 | Thuros Lightfingers | 11 | Rare | Human melee | 4 | Hostile | Backstab | Saknas | 12:Elwynn Forest;18:Crystal Lake;92:Mirror Lake;797:Jerod's Landing |
| 94 | Defias Cutpurse | 5–6 | Normal | Human melee | 30 | Hostile | Backstab | Saknas | 12:Elwynn Forest;89:Mirror Lake Orchard;92:Mirror Lake |
| 99 | Morgaine the Sly | 10 | Rare | Human melee | 1 | Hostile | Backstab, Gouge, Poison | Saknas | 89:Mirror Lake Orchard |
| 103 | Garrick Padfoot | 5 | Normal | Human melee | 1 | Hostile | — | Mappad | 59:Northshire Vineyards |
| 116 | Defias Bandit | 8–9 | Normal | Human melee | 57 | Hostile | Snap Kick | Saknas | 12:Elwynn Forest;18:Crystal Lake;61:Thunder Falls;62:Brackwell Pumpkin Patch;89:Mirror Lake Orchard;92:Mirror Lake;797:Jerod's Landing;798:Ridgepoint Tower |
| 473 | Morgan the Collector | 10 | Normal | Human melee | 1 | Hostile | Backstab, Gouge | Saknas | 62:Brackwell Pumpkin Patch |
| 880 | Erlan Drudgemoor | 8 | Normal | Human melee | 1 | Hostile | — | Saknas | 62:Brackwell Pumpkin Patch |
| 6093 | Dead-Tooth Jack | 11 | Normal | Human melee | 1 | Hostile | — | Saknas | 12:Elwynn Forest |
| 6927 | Defias Dockworker | 8–9 | Normal | Human melee | 3 | Hostile | Snap Kick | Saknas | 797:Jerod's Landing |
| 13159 | James Clark | 12 | Normal | Human melee | 1 | Neutral | — | Saknas; neutral named/quest | 88:Eastvale Logging Camp |
| 474 | Defias Rogue Wizard | 9–10 | Normal | Human caster | 29 | Hostile | Frost Armor, Frostbolt | Saknas | 12:Elwynn Forest;18:Crystal Lake;56:Heroes' Vigil;86:Stone Cairn Lake;92:Mirror Lake;797:Jerod's Landing |
| 881 | Surena Caledon | 9 | Normal | Human caster | 1 | Hostile | Fireball, Frost Armor | Saknas | 62:Brackwell Pumpkin Patch |
| 6846 | Defias Dockmaster | 10 | Normal | Goblin | 1 | Hostile | — | Saknas | 797:Jerod's Landing |
| 97 | Riverpaw Runt | 8–9 | Normal | Gnoll | 26 | Hostile | — | Saknas | 60:Forest's Edge;63:The Stonefield Farm;86:Stone Cairn Lake |
| 100 | Gruff Swiftbite | 12 | Rare | Gnoll | 1 | Hostile | Thrash | Saknas | 60:Forest's Edge |
| 448 | Hogger | 11 | Elite | Gnoll | 5 | Hostile | Head Butt, Pierce Armor, Rushing Charge | Mappad; look/feel parkerat | 60:Forest's Edge |
| 472 | Fedfennel | 12 | Rare | Gnoll | 1 | Hostile | Demoralizing Shout, Nimble Reflexes | Saknas | 86:Stone Cairn Lake |
| 478 | Riverpaw Outrunner | 9–10 | Normal | Gnoll | 58 | Hostile | Thrash | Saknas | 60:Forest's Edge;86:Stone Cairn Lake |
| 46 | Murloc Forager | 9–10 | Normal | Murloc | 60 | Hostile | Drink Minor Potion | Saknas | 12:Elwynn Forest;62:Brackwell Pumpkin Patch;86:Stone Cairn Lake |
| 285 | Murloc | 6–7 | Normal | Murloc | 33 | Hostile | — | Saknas | 18:Crystal Lake |
| 732 | Murloc Lurker | 9–10 | Normal | Murloc | 40 | Hostile | Backstab | Saknas | 12:Elwynn Forest;86:Stone Cairn Lake |
| 735 | Murloc Streamrunner | 6–7 | Normal | Murloc | 19 | Hostile | — | Saknas | 18:Crystal Lake |
| 113 | Stonetusk Boar | 5–6 | Normal | Boar | 47 | Neutral | Rushing Charge | Saknas | 12:Elwynn Forest;57:Fargodeep Mine;63:The Stonefield Farm;64:The Maclure Vineyards;87:Goldshire |
| 330 | Princess | 9 | Normal | Boar | 1 | Neutral | Rushing Charge | Saknas | 62:Brackwell Pumpkin Patch |
| 390 | Porcine Entourage | 7 | Normal | Boar | 2 | Neutral | Rushing Charge | Saknas | 62:Brackwell Pumpkin Patch |
| 524 | Rockhide Boar | 7–8 | Normal | Boar | 90 | Neutral | Rushing Charge | Saknas | 12:Elwynn Forest;62:Brackwell Pumpkin Patch;64:The Maclure Vineyards;797:Jerod's Landing |
| 30 | Forest Spider | 5–6 | Normal | Spider | 17 | Hostile | Poison, Poison Proc (AI) | Saknas | 12:Elwynn Forest;89:Mirror Lake Orchard;92:Mirror Lake |
| 43 | Mine Spider | 8–9 | Normal | Spider | 8 | Hostile | Poison, Poison Proc (AI) | Saknas | 9:Northshire Valley;12:Elwynn Forest |
| 471 | Mother Fang | 10 | Rare | Spider | 1 | Hostile | Poison, Web | Saknas | 12:Elwynn Forest |
| 822 | Young Forest Bear | 8–9 | Normal | Bear | 48 | Hostile | — | Saknas | 12:Elwynn Forest;18:Crystal Lake;54:Jasperlode Mine;57:Fargodeep Mine;60:Forest's Edge;62:Brackwell Pumpkin Patch;86:Stone Cairn Lake;88:Eastvale Logging Camp;91:Tower of Azora;120:Westbrook Garrison;797:Jerod's Landing;798:Ridgepoint Tower |

## Attacker som behöver egen rörelse eller tydlig roll

| Behov | Berörda | Rekommenderad lösning |
|---|---|---|
| Fireball / Frostbolt / försvarsbesvärjelse | Kobold Geomancer, Defias Rogue Wizard, Surena Caledon | Ett enemy-caster-flöde med projectile release vid DS-eventet, avbrott och WoW-effekt. c4160 har läsbar stavattack + ett projektilkast; c2650 har ett enkelt kast. Befintliga Mage-casts är alternativ men spelarflödet är inte ett färdigt NPC-caster-flöde. |
| Charge + närattack | Stonetusk Boar, Rockhide Boar, Princess, Porcine Entourage | Boar behöver egen kroppsrörelse och huvud-hitbox. c3460 är bästa djurmatchningen men alla undersökta träffar har dmgLevel 7/9; originalet passerar inte Common. Ingen automatisk sänkning eller gate-lättnad beslutad. Kort huvudattack med uttrycklig gameplay-adaptation eller annan källa krävs. Princess får egen pattern och eskort, inte ett helt nytt skelett. |
| Spider-bett + gift / nät | Forest Spider, Mine Spider, Mother Fang | Dela attacker per rigg. Gift kopplas till bettet. Mother Fang behöver separat läsbar Web-roll. c3400:s första två melee-klipp är enstaka level-1-träffar; fire-spray/projektil ska inte kopieras. WoW-modellerna har egna AttackUnarmed-klipp som reserv. |
| Björntass + bett | Young Forest Bear | Eget bear-kit. Ingen direkt DSR-björn verifierad. Great Feline-kandidaten har level-3 närattacker och level-10 långa rullningar: inte ett färdigt Common-kit. WoW-björnens egna attack finns; DS-bettreferens kan komplettera den efter motion review. |
| 1H-vapen på kompakt rigg | Murloc, Streamrunner, Forager, Lurker | c2550 för hugg/klubbslag och c2520 för Lurker-dolk. Fyra WoW-vapen: svärd, spiked club, scimitar, dagger. Dela källa men kalibrera vapengrepp och kapsel per variant. |
| Vanlig gnoll-attack | Riverpaw Runt/Outrunner, Gruff, Fedfennel | c2550 har lätta single-hit-klipp. c2810 har två lugna level-2-klubbrörelser som alternativ för större gnoller. Taurus-bossens fulla kit ska inte användas på vanliga gnoller. |
| Snap Kick | Defias Bandit, Defias Dockworker | Kräver identifierad fotattack och fot-hitbox om WoW-förmågan behålls. Befintliga dagger-hugg är inte en verifierad kick. Kandidaten behöver motion-/kontaktkontroll innan val. |
| Backstab / Gouge / Poison | Cutpurse, Thuros, Morgaine, Morgan, Murloc Lurker; Bodyguard | Dela dolkattacker och lägg positionskrav/status på rätt rörelse. Morgaine är HumanFemale. Behöver inte helt egna importer per named fiende. |
| Battle / Demoralizing Shout, Thrash, Pierce Armor, Net | Narg, Fedfennel, Gruff/Outrunner, Miner, Ruklar | Oftast effect/pattern-arbete över delat kit. Ruklar Net behöver läsbar release-rörelse och root-status; Thrash får inte automatiskt öka Common över två attacker. |
| Obeväpnad goblin + summon | Defias Dockmaster | DB equipment_id = 0. Välj obeväpnad Hollow-rörelse och GoblinMale-retarget; hitta inte på ett dolkvapen. Tre Bodyguard-summons finns i hans script. |
| Tvåhandsyxa | Erlan Drudgemoor | DB-vapnet är 2H axe/pick: dela mänsklig rigg men välj 2H-swing (c0000 redan läst av weapon-pipelinen är ett alternativ), inte bara Thug-dagger som slutlig animation. |
| Guard combat | Stormwind/Northshire guards; Expeditionary cliff NPCs | Vakter behöver delat vapen-/skjutkit om NPC-strid ingår. Friendly måste bestämmas av faction; flags==0 gör inte bönder eller lumberjacks till fiender. Dwarf-riggar på klippan är ett separat sent scenariobehov. |

## Målriggar verifierade i WoW-klienten

13 modellfiler används av de 45 typerna. Antalet ben visar varför riggmappning inte kan kopieras efter namn eller kroppsstorlek. Ny rigg = nya bone-index/pivots, inte nödvändigtvis nya DS-källanimationer.

| WoW-modell | Ben | Status |
|---|---:|---|
| Character\Goblin\Male\GoblinMale.m2 | 111 | Ny/ej byggd target-mappning |
| Character\Human\Female\HumanFemale.m2 | 117 | Ny/ej byggd target-mappning |
| Character\Human\Male\HumanMale.m2 | 119 | Befintlig target-rigg; nya källor kan behöva egen bonemap |
| Creature\Boar\Boar.m2 | 57 | Ny/ej byggd target-mappning |
| Creature\DireWolf\DireWolf.m2 | 61 | Ny/ej byggd target-mappning |
| Creature\GnollMelee\GnollMelee.m2 | 60 | Befintlig target-rigg; nya källor kan behöva egen bonemap |
| Creature\Gnoll\gnoll.m2 | 60 | Ny/ej byggd target-mappning |
| Creature\Kobold\Kobold.m2 | 72 | Befintlig target-rigg; nya källor kan behöva egen bonemap |
| Creature\MineSpider\MineSpider.m2 | 91 | Ny/ej byggd target-mappning |
| Creature\Murloc\Murloc.m2 | 62 | Ny/ej byggd target-mappning |
| Creature\Tarantula\Tarantula.m2 | 74 | Ny/ej byggd target-mappning |
| Creature\Wolf\Wolf.m2 | 64 | Befintlig target-rigg; nya källor kan behöva egen bonemap |
| Creature\bear\Bear.m2 | 54 | Ny/ej byggd target-mappning |

**HumanFemale är 117 ben mot HumanMale 119. GoblinMale är 111. Wolf är 64 mot DireWolf 61. Tarantula är 74 mot MineSpider 91.** Riverpaw Runt har Gnoll (60), övriga stridsgnoller GnollMelee (60); lika antal bevisar inte identisk topologi.

## Särskilda spawns och scripts

- **Hogger:** 5 grund-GUID:er, men `pool_creature_template` 448 → pool 1270, max_limit 1. Bevara ett aktivt boss-exemplar; spawnplats väljs från poolen. Handover parkerar hans look/feel.
- **Thuros Lightfingers:** 4 grund-GUID:er, pool 3200, max_limit 1. Inte fyra samtidiga rares.
- **Defias Bodyguard (6866):** summoned av Dockmaster-script 684602, tre summonrader. Level 10, male/female displays 2340/2341. Delat bandit/dolk-kit + Disarm/Backstab; ingen permanent grundspawn att hitta på.
- **Defias Ambusher (583):** summoned av Clara Charles-script 591701. Level 8–9, display 184, equipment 583. Tilldela attackkit från mänsklig melee-familj och håll spawnen scriptstyrd.
- **Imp Minion (12922):** Summon Imp 11939 på Dane Winslow/Cylina Darkheart pekar på denna NPC. Dekorativ/pet-kopplad NPC, ingen vanlig Elwynn-fiende. DB-template är level 46: det är ingen anledning att placera en level-46 aggressiv fiende i startområdet.
- Questscript 74 summon Forlorn Spirit (2044) sker vid Stormwind-koordinater. Kopplat via en questkedja, men inte lokal Elwynn-fiende.
- James Clark (13159) har neutral faction 7, level 12 och long sword. Bevara neutral startreaktion och quest/angreppsutlöst konflikt.
- Expeditionsdwarves (14390/14393) finns högt ovanför den vanliga spelbara skogen, nära Elwynn/Burning Steppes-gränsen. Behåll dem som scenariodata; de är inte låglevelfiender.
- **Uteslutna zonförväxlingar:** Riverpaw Gnoll (117), Riverpaw Mystic (453), Riverpaw Scout (500), Murloc Coastrunner (126) och Redridge-gnoller har inga valda Elwynn-grundspawns i denna datakälla. Använd inte hela släktens katalog som zonroster.

## Alla övriga vanliga NPC:er och djur

128 typer. Spawna på DB-plats med rätt display/skin/scale/utrustning, service och rörelsetyp. Civil NPC kräver normalt sina WoW-idle/walk/emote-klipp; ingen unik DSR-attack. Flera flagglösa friendly NPC:er kräver att nuvarande `world::kind_of` får en fullare klassificering.

| Entry | Namn | Level | Roll / reaktion | Områden från terrängen |
|---:|---|---:|---|---|
| 810 | Aaron | 1 | ambient/vakt/djur; Friendly | 87:Goldshire |
| 1632 | Adele Fielder | 22 | trainer; Friendly | 87:Goldshire |
| 1215 | Alchemist Mallory | 26 | quest, trainer; Friendly | 12:Elwynn Forest |
| 2046 | Andrew Krighton | 8 | vendor; Friendly | 87:Goldshire |
| 844 | Antonio Perelli | 35 | vendor; Friendly | 12:Elwynn Forest |
| 11996 | Ashley Bridenbecker | 1 | ambient/vakt/djur; Friendly | 12:Elwynn Forest |
| 246 | Auntie Bernice Stonefield | 6 | quest; Friendly | 63:The Stonefield Farm |
| 465 | Barkeep Dobbins | 10 | vendor; Friendly | 87:Goldshire |
| 247 | Billy Maclure | 1 | quest; Friendly | 64:The Maclure Vineyards |
| 797 | Bo | 1 | ambient/vakt/djur; Friendly | 87:Goldshire |
| 7385 | Bombay | 1 | ambient/vakt/djur; Neutral | 12:Elwynn Forest |
| 151 | Brog Hamfist | 10 | vendor; Friendly | 87:Goldshire |
| 152 | Brother Danil | 5 | vendor; Friendly | 9:Northshire Valley |
| 952 | Brother Neals | 50 | quest; Friendly | 9:Northshire Valley |
| 951 | Brother Paxton | 27 | quest; Friendly | 9:Northshire Valley |
| 925 | Brother Sammuel | 5 | quest, trainer; Friendly | 9:Northshire Valley |
| 927 | Brother Wilhelm | 11 | quest, trainer; Friendly | 87:Goldshire |
| 12376 | Brown Horse | 1–2 | ambient/vakt/djur; Friendly | 88:Eastvale Logging Camp |
| 805 | Cameron | 1 | ambient/vakt/djur; Friendly | 87:Goldshire |
| 6368 | Cat | 1 | critter; Neutral | 12:Elwynn Forest |
| 12375 | Chestnut Mare | 1–2 | ambient/vakt/djur; Friendly | 88:Eastvale Logging Camp |
| 620 | Chicken | 1 | quest, critter; Neutral | 12:Elwynn Forest;87:Goldshire;88:Eastvale Logging Camp;797:Jerod's Landing |
| 5917 | Clara Charles | 1–2 | ambient/vakt/djur; Friendly | 12:Elwynn Forest |
| 54 | Corina Steele | 10 | vendor; Friendly | 87:Goldshire |
| 7384 | Cornish Rex | 1 | ambient/vakt/djur; Neutral | 12:Elwynn Forest |
| 2442 | Cow | 5 | critter; Neutral | 12:Elwynn Forest;18:Crystal Lake;54:Jasperlode Mine;57:Fargodeep Mine;60:Forest's Edge;62:Brackwell Pumpkin Patch;63:The Stonefield Farm;86:Stone Cairn Lake;88:Eastvale Logging Camp;91:Tower of Azora;120:Westbrook Garrison;798:Ridgepoint Tower |
| 6374 | Cylina Darkheart | 11 | quest, vendor; Friendly | 87:Goldshire |
| 804 | Dana | 1 | ambient/vakt/djur; Friendly | 87:Goldshire |
| 6373 | Dane Winslow | 8 | quest, vendor; Friendly | 9:Northshire Valley |
| 958 | Dawn Brightstar | 35 | vendor; Friendly | 91:Tower of Azora |
| 883 | Deer | 5 | critter; Neutral | 9:Northshire Valley;12:Elwynn Forest;34:Echo Ridge Mine;57:Fargodeep Mine;59:Northshire Vineyards;60:Forest's Edge;62:Brackwell Pumpkin Patch;64:The Maclure Vineyards;86:Stone Cairn Lake;87:Goldshire;88:Eastvale Logging Camp;797:Jerod's Landing |
| 963 | Deputy Rainer | 10 | quest; Friendly | 120:Westbrook Garrison |
| 823 | Deputy Willem | 18 | quest; Friendly | 9:Northshire Valley |
| 190 | Dermot Johns | 5 | vendor; Friendly | 9:Northshire Valley |
| 6367 | Donni Anthania | 5 | vendor; Friendly | 12:Elwynn Forest |
| 1250 | Drake Lindgren | 10 | vendor; Friendly | 88:Eastvale Logging Camp |
| 459 | Drusilla La Salle | 5 | quest, trainer; Friendly | 9:Northshire Valley |
| 196 | Eagan Peltskinner | 3 | quest; Friendly | 9:Northshire Valley |
| 1975 | Eastvale Lumberjack | 7–8 | ambient/vakt/djur; Friendly | 88:Eastvale Logging Camp |
| 11328 | Eastvale Peasant | 6–7 | ambient/vakt/djur; Friendly | 88:Eastvale Logging Camp |
| 1103 | Eldrin | 22 | trainer; Friendly | 88:Eastvale Logging Camp |
| 6749 | Erma | 30 | stable; Friendly | 87:Goldshire |
| 14390 | Expeditionary Mountaineer | 36–40 | ambient/vakt/djur; Friendly | 12:Elwynn Forest |
| 14393 | Expeditionary Priest | 37–42 | ambient/vakt/djur; Friendly | 12:Elwynn Forest |
| 6774 | Falkhaan Isenstrider | 10 | quest; Friendly | 9:Northshire Valley |
| 890 | Fawn | 1 | critter; Neutral | 9:Northshire Valley;12:Elwynn Forest;60:Forest's Edge;87:Goldshire;88:Eastvale Logging Camp |
| 255 | Gerard Tiller | 6 | quest; Friendly | 64:The Maclure Vineyards |
| 1213 | Godric Rothgar | 5 | vendor; Friendly | 9:Northshire Valley |
| 248 | Gramma Stonefield | 3 | quest; Friendly | 63:The Stonefield Farm |
| 12423 | Guard Roberts | 7 | ambient/vakt/djur; Friendly | 18:Crystal Lake |
| 261 | Guard Thomas | 30 | quest; Friendly | 12:Elwynn Forest |
| 6306 | Helene Peltskinner | 12 | trainer; Friendly | 87:Goldshire |
| 6172 | Henze Faulk | 10 | quest; Friendly | 56:Heroes' Vigil |
| 1218 | Herbalist Pomeroy | 10 | trainer; Friendly | 12:Elwynn Forest |
| 894 | Homer Stonefield | 5 | vendor; Friendly | 63:The Stonefield Farm |
| 385 | Horse | 5 | critter; Friendly | 88:Eastvale Logging Camp |
| 295 | Innkeeper Farley | 30 | quest, vendor; Friendly | 87:Goldshire |
| 78 | Janos Hammerknuckle | 5 | vendor; Friendly | 9:Northshire Valley |
| 383 | Jason Mathers | 7 | quest; Friendly | 18:Crystal Lake |
| 806 | John | 1 | ambient/vakt/djur; Friendly | 87:Goldshire |
| 915 | Jorik Kerridan | 5 | quest, trainer; Friendly | 9:Northshire Valley |
| 811 | Jose | 1 | ambient/vakt/djur; Friendly | 87:Goldshire |
| 796 | Joshua | 1 | ambient/vakt/djur; Friendly | 87:Goldshire |
| 258 | Joshua Maclure | 5 | vendor; Friendly | 64:The Maclure Vineyards |
| 384 | Katie Hunter | 10 | quest, vendor; Friendly | 88:Eastvale Logging Camp |
| 917 | Keryn Sylvius | 11 | quest, trainer; Friendly | 87:Goldshire |
| 198 | Khelden Bremen | 5 | quest, trainer; Friendly | 9:Northshire Valley |
| 11979 | Kim Bridenbecker | 25 | ambient/vakt/djur; Friendly | 12:Elwynn Forest |
| 3937 | Kira Songshine | 12 | vendor; Friendly | 87:Goldshire |
| 11072 | Kitta Firewind | 44 | trainer; Friendly | 91:Tower of Azora |
| 74 | Kurran Steele | 10 | vendor; Friendly | 87:Goldshire |
| 1651 | Lee Brown | 8 | trainer; Friendly | 18:Crystal Lake |
| 807 | Lisa | 1 | ambient/vakt/djur; Friendly | 87:Goldshire |
| 911 | Llane Beshere | 5 | quest, trainer; Friendly | 9:Northshire Valley |
| 913 | Lyria Du Lac | 12 | quest, trainer; Friendly | 87:Goldshire |
| 244 | Ma Stonefield | 3 | quest; Friendly | 63:The Stonefield Farm |
| 795 | Mark | 1 | ambient/vakt/djur; Friendly | 87:Goldshire |
| 395 | Markus | 1 | ambient/vakt/djur; Friendly | 12:Elwynn Forest |
| 240 | Marshal Dughan | 25 | quest; Friendly | 87:Goldshire |
| 294 | Marshal Haggard | 20 | quest; Friendly | 88:Eastvale Logging Camp |
| 197 | Marshal McBride | 20 | quest; Friendly | 9:Northshire Valley |
| 794 | Matt | 1 | ambient/vakt/djur; Friendly | 18:Crystal Lake |
| 906 | Maximillian Crowe | 10 | trainer; Friendly | 87:Goldshire |
| 251 | Maybell Maclure | 2 | quest; Friendly | 64:The Maclure Vineyards |
| 6778 | Melika Isenstrider | 11 | quest; Friendly | 87:Goldshire |
| 11940 | Merissa Stilwell | 5 | quest; Friendly | 9:Northshire Valley |
| 2329 | Michelle Belle | 11 | trainer; Friendly | 87:Goldshire |
| 9296 | Milly Osworth | 2 | quest; Friendly | 9:Northshire Valley |
| 959 | Morley Eberlein | 10 | vendor; Friendly | 91:Tower of Azora |
| 1642 | Northshire Guard | 55 | ambient/vakt/djur; Friendly | 9:Northshire Valley;12:Elwynn Forest |
| 11260 | Northshire Peasant | 1 | ambient/vakt/djur; Friendly | 9:Northshire Valley |
| 7382 | Orange Tabby | 1 | ambient/vakt/djur; Neutral | 12:Elwynn Forest |
| 250 | Pa Maclure | 6 | quest; Friendly | 64:The Maclure Vineyards |
| 5406 | Palomino | 1–2 | ambient/vakt/djur; Friendly | 87:Goldshire |
| 5405 | Pinto | 1–2 | quest; Friendly | 88:Eastvale Logging Camp |
| 375 | Priestess Anetta | 5 | quest, trainer; Friendly | 9:Northshire Valley |
| 377 | Priestess Josetta | 13 | quest, trainer; Friendly | 87:Goldshire |
| 1645 | Quartermaster Hicks | 10 | vendor; Friendly | 120:Westbrook Garrison |
| 1249 | Quartermaster Hudson | 10 | vendor; Friendly | 120:Westbrook Garrison |
| 721 | Rabbit | 1 | critter; Neutral | 9:Northshire Valley;12:Elwynn Forest;34:Echo Ridge Mine;57:Fargodeep Mine;59:Northshire Vineyards;60:Forest's Edge;62:Brackwell Pumpkin Patch;63:The Stonefield Farm;64:The Maclure Vineyards;87:Goldshire;88:Eastvale Logging Camp;120:Westbrook Garrison;797:Jerod's Landing;798:Ridgepoint Tower |
| 1198 | Rallic Finn | 18 | vendor; Friendly | 88:Eastvale Logging Camp |
| 4732 | Randal Hunter | 10 | trainer; Friendly | 88:Eastvale Logging Camp |
| 6121 | Remen Marcot | 10 | quest; Friendly | 87:Goldshire |
| 241 | Remy "Two Times" | 5 | quest; Friendly | 87:Goldshire |
| 11994 | Rob Bridenbecker | 25 | ambient/vakt/djur; Friendly | 12:Elwynn Forest |
| 278 | Sara Timberlain | 5 | quest; Friendly | 88:Eastvale Logging Camp |
| 955 | Sergeant De Vries | 10 | vendor; Friendly | 120:Westbrook Garrison |
| 1949 | Servant of Azora | 8–9 | quest; Friendly | 91:Tower of Azora |
| 1933 | Sheep | 3 | critter; Neutral | 12:Elwynn Forest;62:Brackwell Pumpkin Patch;63:The Stonefield Farm;86:Stone Cairn Lake;87:Goldshire;88:Eastvale Logging Camp;92:Mirror Lake |
| 7381 | Silver Tabby | 1 | ambient/vakt/djur; Neutral | 12:Elwynn Forest |
| 514 | Smith Argus | 24 | quest, trainer; Friendly | 87:Goldshire |
| 6491 | Spirit Healer | 60 | Spirit Healer; Friendly | 9:Northshire Valley;12:Elwynn Forest;88:Eastvale Logging Camp |
| 68 | Stormwind City Guard | 55 | ambient/vakt/djur; Friendly | 12:Elwynn Forest |
| 1423 | Stormwind Guard | 22–25 | ambient/vakt/djur; Friendly | 12:Elwynn Forest;60:Forest's Edge;87:Goldshire;120:Westbrook Garrison;798:Ridgepoint Tower |
| 10616 | Supervisor Raelen | 15 | quest; Friendly | 88:Eastvale Logging Camp |
| 14561 | Swift Brown Steed | 1–2 | ambient/vakt/djur; Friendly | 88:Eastvale Logging Camp |
| 14559 | Swift Palamino | 1–2 | ambient/vakt/djur; Friendly | 88:Eastvale Logging Camp |
| 14560 | Swift White Steed | 1–2 | ambient/vakt/djur; Friendly | 88:Eastvale Logging Camp |
| 1650 | Terry Palin | 11 | vendor; Friendly | 88:Eastvale Logging Camp |
| 66 | Tharynn Bouden | 10 | vendor; Friendly | 87:Goldshire |
| 313 | Theocritus | 24 | quest; Friendly | 91:Tower of Azora |
| 3935 | Toddrick | 10 | vendor; Friendly | 87:Goldshire |
| 1430 | Tomas | 10 | trainer; Friendly | 87:Goldshire |
| 252 | Tommy Joe Stonefield | 2 | quest; Friendly | 63:The Stonefield Farm |
| 896 | Veldan Lightfoot | 10 | vendor; Friendly | 120:Westbrook Garrison |
| 5403 | White Stallion | 1–2 | ambient/vakt/djur; Friendly | 9:Northshire Valley;87:Goldshire |
| 253 | William Pestle | 6 | quest; Friendly | 87:Goldshire |
| 328 | Zaldimar Wefhellt | 13 | quest, trainer; Friendly | 87:Goldshire |

## Eventtyper: inventerade, separata från baszonen

33 ytterligare typer. Event-GUID:erna får inte bli permanenta baszons-NPC:er. Särskilt Scourge invasion-fienderna behöver egna kit om eventet implementeras. De övriga event-NPC:erna är huvudsakligen Darkmoon Faire, revelers och ambientdjur.

| Entry | Namn | Level | Event-spawnplatser |
|---:|---|---:|---:|
| 798 | Solomon | 1 | 1 |
| 799 | Kevin | 1 | 1 |
| 800 | Kyle | 1 | 1 |
| 801 | Eric | 1 | 1 |
| 802 | Jay | 1 | 1 |
| 10445 | Selina Dourman | 30 | 1 |
| 14822 | Sayge | 55 | 1 |
| 14823 | Silas Darkmoon | 61 | 1 |
| 14827 | Burth | 60 | 1 |
| 14828 | Gelvas Grimegate | 25 | 1 |
| 14829 | Yebb Neblegear | 55 | 1 |
| 14832 | Kerri Hicks | 55 | 1 |
| 14833 | Chronos | 55 | 1 |
| 14841 | Rinling | 55 | 1 |
| 14844 | Sylannia | 35 | 1 |
| 14845 | Stamp Thunderhorn | 35 | 1 |
| 14846 | Lhara | 35 | 1 |
| 14847 | Professor Thaddeus Paleo | 35 | 1 |
| 14849 | Darkmoon Faire Carnie | 25 | 10 |
| 14860 | Flik | 4 | 1 |
| 14864 | Khaz Modan Ram | 5 | 2 |
| 14865 | Felinni | 10 | 1 |
| 14866 | Flik's Frog | 5 | 1 |
| 14867 | Jubjub | 5 | 1 |
| 14868 | Hornsley | 8 | 1 |
| 14869 | Pygmy Cockatrice | 1 | 4 |
| 14871 | Morja | 12 | 1 |
| 15218 | Faire Cannon Trigger | 60 | 1 |
| 15303 | Maxima Blastenheimer | 35 | 1 |
| 15565 | Elder Stormbrow | 60 | 1 |
| 15760 | Winter Reveler | 30–40 | 2 |
| 16422 | Skeletal Soldier | 6–7 | 23 |
| 16423 | Spectral Apparition | 6–7 | 21 |

Skeletal Soldier (16422) → BoneGolem, Spectral Apparition (16423) → Ghost. Om dessa byggs: undersök c2910/c2900 respektive c2670/c2680. Dessa fyra källor är **inte surveyed eller valda** i detta steg. Faire Cannon Trigger är en osynlig trigger, inte en stridsfiende trots hostile faction.

## Rekommenderad byggordning

1. Generalisera Northshire-urvalet till Elwynn area 12, hantera WMO-gränsen, faction, displayvarianter, alternativslots, pooler och scriptspawns. Återanvänd DB-placeringar, path/radius och 0.5-density med quest floors. Ny zonimport skrivs vid sidan av den levande Northshire-cachen tills verifierad.
2. Koppla nya enkla speciesrader: Miner/Tunneler, Mangy/Gray Wolf, mänskliga Cutpurse/Bandit/namngivna melee. Female/DireWolf-varianter lämnas inte tyst på fel skelett.
3. Bygg HumanFemale-riggen och enemy-casts; gör Kobold Geomancer + Rogue Wizard + Surena med Fireball/Frostbolt/Frost Armor och avbrott.
4. Provretargeta vanligt gnoll-kit och Murloc 1H-kit. Kontrollera riktning, vapenhand, root height, foot contact, death end-pose och korta fightsekvenser i rörelse.
5. Provretargeta Boar och båda Spider-modellerna. Bestäm explicit lösning för Common-gränsen på boar. Bear får sin separata lösning; ingen DS-katt deklareras färdig utan motion review.
6. Named/rare-förmågor, Dockmaster/Bodyguard, Clara-ambush, Princess-eskort, questkopplingar och loot. Återkom till Hoggers parkerade look/feel. Events/klippscenariot hålls som egna senare steg.

Ingen ny vanlig fiende ska ärva ett komplett boss-moveset. Common: högst två attacker, en träffperiod per attack, inga boss-barrager. Tidigare kobold-kit är Legacy enligt befintligt undantag. Återanvändning av en källa ska inte ge alla Elwynn-fiender samma pattern, reach eller effekt.

## Filer och reproduktion

Rådata och installationshärledda survey-dumpar finns i gitignored `cache/elwynn-survey/`: `roster.tsv`, `spawns.tsv`, `models.tsv`, `rigs.tsv`, `templates-extra.tsv`, `creature-spells.tsv`, `ai-events.tsv`, `ai-scripts.tsv`, `pools-by-entry.tsv`, `quests.tsv`, `quest-connections.tsv`, `dsr-c*.txt`. `npc-mapping.tsv` är 14 oktobers snapshot med 207 template-typer; Doc Mixilpixil utesluts dessutom efter WMO-kontrollen. `combat-mapping.tsv` ger familj, DSR-kandidat, attackbehov och områden för de 45 vanliga stridstyperna. `attack-candidates.txt` samlar mätta klippfönster för provretargets. `survey.rs`, `models.rs`, `rigs.rs` är researchverktyg och inte nya gameplaymoduler.

Surveyverktygen tar TSV-/install-/utdatakatalog som argument. DSR: `wow-souls-dsr survey --chr cNNNN --npc NNNN00 --dsr <egen install>`. Klippfönster och dmglevel är mätta men retarget-passning är fortfarande ett experiment.

Verifierat: 45/45 stridstyper har familjmappning; efter WMO-korrigeringen 173 = 45 + 128 vanliga typer; 33 event-only; samtliga display IDs kunde lösas; 13 stridsmodellfiler kunde läsas med ben/animationer. Den ursprungliga kartläggningen ändrade ingen produktionskod eller levande world-/DSR-cache. Implementationens tester och status finns i den separata loggen för 15 oktober.

## Externa korsreferenser

- [Souls Modding: DS1 character identities](https://soulsmodding.com/doku.php?id=ds1-refmat%3Acharacter-list). Använd endast DS1-listan: samma cNNNN-id betyder något annat i andra Souls-spel.
- [Kobold Geomancer](https://classicdb.ch/?npc=476), [Riverpaw Outrunner](https://classicdb.ch/?npc=478), [Ruklar the Trapper](https://classicdb.ch/?npc=60), [Defias Dockmaster](https://classicdb.ch/?npc=6846), [James Clark](https://classicdb.ch/?npc=13159): Elwynn-korskontroll.
- [Jasper Fel](https://classicdb.ch/?npc=1325): hör till Stormwind och uteslöts från den terrängbaserade Elwynn-listan.
- [Doc Mixilpixil](https://classicdb.ch/?npc=7207): Stormwind, ytterligare WMO-avvikelse verifierad 15 oktober (GUID 79782).


