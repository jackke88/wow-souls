# Familje-AI — implementation och spelkontroll

Jacob godkände hela mappningen 2026-10-16. Arbetet fortsatte efter midnatt den 17:e.
Design: `../plans/2026-10-16-enemy-family-ai-design.md`.

## Infört

- `behavior.rs`: 15 roller med separata beslut för sikta/rusa, jaga/pressa, korta utfall,
  flank, samlas/rusa, bakhåll, ändligt avståndsskapande och ledare/eskort.
- Alla 47 `species::ROSTER`-poster har en explicit roll. Hogger och Taurus-jämförelsen
  behåller den separata befintliga bossvägen.
- `npc::think` använder familjens beslut före attackstart. Närhet kan inte köra över en
  förberedelse. Repositionering fungerar även under befintlig cooldown.
- Avståndsgap mellan bett och utfall leder till fortsatt närmande. Förflyttning provar
  mark/väggar; magiker får en siktkontroll och en begränsad undanmanöver vid närhot.
- Vanliga lokala anfallsstarter prioriteras med väntetid och stabil identitet, max två anfallare
  och max en aktiv flockvarg. Övriga medlemmar får rollens uppgift. Döda påverkar inte kön.
- Befintliga kedjor, Net/Web, latch/hop, klippklockor, root motion, HP, skada, poise,
  hit windows, loot, patruller/spawns, leash och respawn fortsätter via sina befintliga system.
- Profilerna/reglerna är vanlig projektkod. Ingen Lua-tolk eller DSR-/ER-installation behövs.
  Detta är referensfasen enligt `../SOURCE-TO-ORIGINAL.md`, inte ett besked om att alla
  befintliga DSR-baserade animationer ersatts med egna.

## Tester och kontroller

26 nya beteendetester täcker mappning, olika sekvenser, riktningsförberedelse, fasta
repositioneringspunkter, attackavståndsgap, reaktionstid, fångbar caster, ledare/eskort,
fairness/query-order, giltiga rörelsenamn och markkant. Fyra kör det riktiga Bevy NPC-systemet.
Första vildsvinstestet visade rött före förberedelsereglerna och grönt efteråt.

Workspace med installationsdata aktiverade: **1 409 tester passerade** efter rättningarna nedan
(parallella uppgifter har också lagt till tester). Format, strikt workspace/all-target Clippy och
bygge passerade. Slutloggar: `cache/family-ai/{tests,clippy,fmt,build}-accepted.log`.

## Rättat efter första filmerna

- Väntande vargar vandrade över gränsen mellan bett och utfall och bytte då fas varje frame.
  Väntpunkten ligger nu inom yttre anfallsräckvidden och är fast medan en annan har turen.
  Samma regression täcker väntande kobolds och lätta Defias.
- Rogue Wizard stannade i gapet mellan bolt och swat: dess gamla fem yards stoppavstånd
  spärrade närmandet. Familjebeslutet får nu nå den verkliga närstridsräckvidden och caster
  behåller Approach i ett avståndsgap. Regressionen kör `npc::think` med projektets riktiga kit;
  den visade rött före rättningen. HP/skada/klipp/räckviddstabeller ändrades inte.
- Geomancer växlade mellan Approach/Engage vid exakt sju yards när spelaren backade.
  Det sköt upp nästa kast. Magiker börjar nu följa först bortom 7,5 yards och går in till
  6,5 innan de stannar; en regression följer båda riktningarna över dessa gränser.
- Pågående kombos och redan köade Net/Web-specialer fortsätter sin befintliga kontrollväg;
  anfallsbudgeten är inte en avbrytare för dem eller för Hoggers separata bosslogik.
- Reservvägen utan animationskit köar nu på sin verkliga procedurala räckvidd i stället för
  det saknade kitets tabell. En NPC-regression kontrollerar att den verkligen startar ett anfall.

## Spelkontroll

Före/efter för kobold (Miner), Stonetusk och björn är filmade 14 s vardera, både från
spelkameran och framifrån; tät kontaktserie granskad för efterversionen. De visar korta
koboldutfall/undanmanövrer, vildsvinets inriktning/charge/gore/återhämtning och björnens
närtryck. Därefter passerade 22 grundscenarier (16–28 s, 20 fps), följda av sex riktade
uppföljningar för magiker och Net/Web. Alla avslutades med `WOWSOULS_CHECK ok` och riktiga anfall.

- Varg, gnoll, murloc och kobold med tre medlemmar: alla tre får anfall/träffar. Vargens
  fasbyten gick från 1 036 i felversionen till 35 i samma 18 s test efter rättningen.
- Magikerns backning: 257 fasbyten före avståndsbandet, 13 efteråt, med fortsatta kast.
  Även framifrån och med spelarens slag/rullningar. Nära Rogue Wizard går över till swat.
- Defias: korta Cutpurse-utbyten, Bandit-spark mot faktisk guard, Erlans förberedda yxhugg.
- Garrick: flank/duell, egna befintliga kedjor, stagger och död efter spelarens attacker.
- Dockmaster: tre Bodyguards, ledaren bakom gruppen; vakternas Disarm/Backstab kvar.
- Spindel: leap/latch/hop och rullningsfall. Mother Fang behåller Poison och Web.
  På längre avstånd kastade Ruklar Net vid 4,9 yd och Mother Fang Web vid 10,4 yd;
  båda projektilerna träffade och höll spelaren.
- Prowler når sitt korta bett. Björn följer en backande spelare. Murloc-gruppen
  återupptar strid efter den verkliga reset-signalen.

Slutändringen för reservvägen ändrar inte kitladdade aktörers avstånd. Profiler/filmer:
`before`/`after` = kärnjämförelse, `fix` = grundserie, `stable` = magikerns rättade avståndsband,
`accepted` = samma ordinarie beteenden plus rättad reservväg. Mappningen anger nu `active`
för de 46 vanliga posterna och `kept_existing_boss` för Hogger.
Även slutbinären `accepted` filmades med tre vargar i 12 s: CHECK ok, åtta anfallsstarter,
22 fasbyten och träffar från alla tre individerna.
Runtime-loggarna har det befintliga asset-felet för `inv_misc_emptyflask_01@cc.blp`, även i
förebygget. Inga andra ERROR/panic-rader i de 29 avslutade grund-/uppföljnings-/slutkontrollerna.

Instrument: `cache/family-ai/run.ps1`, `wave.ps1`, `caster-wave.ps1`; isolerade saves och manifests, level-11
testkaraktär, project text animation library, DSR-/ER-installationer satta till saknade sökvägar.
Inga användarsparfiler eller originalinstallationer ändras. Inget uppladdat eller committat.
Evidens: `family-ai/` under evidensroten i HANDOVER (README, utvalda filmer, täta bildserier,
samtliga grund-/uppföljningsloggar, scenariotabeller och binärhashar).

## Omfattning och kvarvarande bedömning

Automatiska mappnings- och kitkontroller täcker alla 47 rosterposter. Filmerna provar
representanter för de 14 vanliga rollerna och utvalda namngivna undantag; Hoggers separata
bossarbete ändras inte här. Det är inte en filmad kontroll av varje variant, klass och terräng.

Markproberna är lokal styrning via den befintliga motorn, inte en ny navmesh eller en full
vägsökare runt byggnader. Markkant provas i kollisionsfixture. Helhetsbalansen behöver Jacobs
speltest: oförändrade skadevärden kan ändå kännas annorlunda när grupper och anfallstakt ändras.
