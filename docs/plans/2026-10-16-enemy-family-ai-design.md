# Familjernas strids-AI — kartläggning och designförslag

**Datum:** 2026-10-16. **Status:** godkänt av Jacob 2026-10-16; infört och verifierat 2026-10-17 enligt resultatloggen.
**Omfattning:** samtliga 47 poster i `species::ROSTER`, inklusive namngivna varianter och summons.
Hogger dokumenteras som befintlig separat boss; hans omarbete tillhör en annan uppgift.

## Målet

Spelaren ska känna igen en familj på dess beslut: hur den närmar sig, väljer ett tillfälle,
anfaller, svarar på motstånd och samarbetar. En björn pressar, ett vildsvin riktar in en rusning,
en spindel förbereder ett språng och en varg söker en flank.

Vi behåller de fungerande DSR-animationerna och nuvarande HP, skada, poise, attackfönster,
attackernas root motion och belöningar i första passet. ER är här en referens för beslutslogik.
Alla fiender aggrar fortfarande på syn. DB-styrda patruller, vandring och spawnplatser består.

## Vad undersökningen faktiskt visar

- `footwork.rs` har en gemensam följd av Close/Circle/Hold/Back för alla familjer.
- `species.rs` ger vildsvin, björnar och skogsspindlar `Temper::WOLF`; deras varianter ärver den.
  Gnolls, murlocs och Dockmaster får `Temper::THUG`. Magiker ärver en närstridsprofil.
- `npc::think` använder samma attackval och ett gemensamt tak på två engagerade fiender inom
  tio yards, oavsett familj. Pausen kan avbrytas av att spelaren kommer inom räckhåll.
- `pattern.rs` säger uttryckligen att tabellerna är handskrivna och att ghoulens Lua-AI inte läses.
  Vi har alltså importerade rörelser, men ingen familjevis importerad beslutslogik.
- 22 DSR- och 15 ER-skript har extraherats och avkodats från de lokala installationerna.
  Alla 37 avkodningar lyckades. Utvalda beslut, reaktioner och efterattackfunktioner har granskats.
  Det är ett representativt urval, inte en inventering av spelens alla AI-skript.

Spelen har många individuella Battle GOAL-skript med delade hjälpfunktioner. Avstånd, vinkel,
timers, hälsa, effekter, grupproller och events påverkar valen. `NpcThinkParam` och kartans events
kan aktivera eller stänga av grenar. Ett skript visar vilka beteenden som stöds; dess exakta
konfiguration i varje originalmöte är inte fastställd av denna Lua-granskning.

## Rekommenderad lösning

Ge varje stridsroll en egen tillståndslogik. Dela kollisionssäker förflyttning, perception,
leash, attackuppspelning och gruppsamordning. Familjen väljer **både** rörelse och om/vilken
attack som får starta. Det hindrar det nuvarande närhetsvillkoret från att köra över en familjs
förberedelse eller reträtt. En träff/stagger/death fortsätter avbryta enligt spelets befintliga regler.

Tre alternativ har vägts:

1. **Separata beteenden i vår AI, med referensspelen som underlag — rekommenderas.** Tydliga
   skillnader, återanvänder fungerande kit och fungerar utan originalspel vid körning.
2. **Fler `Temper`-värden.** Liten ändring men samma cirkel/paus/reträtt-sekvens består.
3. **Köra originalens Lua/GOAL-system.** Kräver betydligt fler motorfunktioner, effekter,
   think-parametrar och eventkopplingar. För stor omväg för vårt mål om egna ersättningar.

Följande beteenden är våra införda anpassningar av de granskade referenserna. Spelkontroller
och deras omfattning redovisas i resultatloggen. Källornas meter, probabilities och långa combos kopieras
inte rakt av till WoW-kropparnas räckvidd och våra nuvarande attackbudgetar.

## Mappning

| Vår familj/roll | Huvudreferens, AI-ID | Föreslagen rytm och spelbar skillnad |
|---|---|---|
| **Kobolds** | ER Imp `308000`; tyngre variant `308010` | Kort positionering → snabbt inkliv → 1–2 befintliga slag → sök en säker plats. Övriga kobolds håller en vinkel eller väntar på en lucka. Ingen ständig omloppsbana. |
| **Vargar** | ER Wolf `407000`, DSR Black Dog `334000` som jämförelse | Flockroller: en binder upp, en söker sidan, övriga håller avstånd. Kort rusning/bett och sedan ny flank. Vargar får behålla den tydliga cirklande jaktidentiteten. |
| **Defias med dolk/lätt svärd** | DSR Hollow Thief `252000` | Sök spelarens oskyddade sida → kort anfall → sidbyte. Reaktioner på ett avslutat spelarhugg får en läsbar fördröjning; inga perfekta svar på knapptryckningar. |
| **Defias slagskämpar** | DSR Hollow Warrior `254000` | Gå fram och håll närkamp, använd befintlig spark när avstånd och kit tillåter, återhämta på plats. Korta offensiva sekvenser i stället för dolktjuvens undanmanövrer. |
| **Erlan/tung yxa** | DSR club-Hollow `253000` | Närma sig bestämt → tydligt förberett tungt hugg → synlig återhämtning. Skydd mot hugg ändrar nästa angreppsval inom befintligt kit. |
| **Gnolls** | DSR Hollow Warrior/Soldier `254000`/`255000` | Framåtriktat grupptryck, kort slagväxling, håll mark efter slag. Sidosteg är korta korrigeringar. Gruff/Fedfennel behåller befintlig Thrash-variation. Sköldgrenar i källan används bara om vår utrustning stöder dem. |
| **Murlocs** | ER Demi-Human `410000`/`410020` | Samla gruppen → en samordnad kort rusning → korta slag → samla om vid en lokal punkt. Streamrunner når sidan, Lurker söker bakhåll. Flocken flyttar sin samlingspunkt i stället för att alla går runt spelaren. |
| **Vildsvin** | DSR Armored Tusk `346000` | Vänd kroppen mot målet → förbered → befintlig rak charge/gore → bromsa och vänd om. Attackens befintliga låsning/root motion respekteras. Ingen vargflank mellan rusningarna. |
| **Björnar** | ER Runebear `463000` | Stadigt gå in och håll tryck med befintlig bite/swipe; om spelaren går undan följer björnen. Kraftiga angrepp lämnar återhämtning. RuneBears grabs, jättecombos och bossvärden krävs inte för denna rytm. |
| **Spindlar** | DSR Crag-Spider `340000` | Stanna och rikta in → befintligt leap → latch/bite → befintligt hop_back → ny väntan. Tydliga språngtillfällen ersätter cirklandet. Forest/Mine/Mother Fang får respektive nuvarande räckvidd. |
| **Magiker** | DSR Oolacile/Bloathead Sorcerer `416000` | Håll kastavstånd och fri sikt → kasta → flytta om vid hot. I närkamp: befintlig swat/slag och en begränsad reträtt. En trängd magiker går att fånga; avståndshållning får inte bli evig flykt. |
| **Dockmaster** | Egen ledarroll; ER:s granskade grupprollsprinciper som referens | Kalla befintliga tre Bodyguards, stå bakom dem och försvara sig med befintliga slag. Bli mer framåtriktad när skyddet saknas. Summons antal/livslängd ändras inte. |
| **Bodyguards** | DSR soldattryck `254000`/`255000` + egen eskortroll | Ställ sig mellan spelaren och Dockmaster; gå till anfall när ledaren hotas. Sök backstab-vinkel när skyddsuppgiften tillåter. |
| **Garrick** | DSR Assassin/Painting Guardian `240000` | Egen duellrytm: håll bladavstånd, byt sida vid en öppning, använd befintliga kedjor och låt en miss kunna straffas. |
| **Hogger** | DSR Artorias `410000`, redan aktiv referens | Förblir separat bosslogik och den andra uppgiftens ansvar. Ingen automatisk omläggning till vanlig gnoll-AI. |

**Imp-förbehåll:** `308000` har själv reträtt, sidoförflyttning och grupproller. Ett direkt byte
till hela Imp-skriptet skulle alltså också kunna ge cirklar. Det vi väljer är dess korta
angrepp och opportunistiska grupproller. Vägg-/takhäng, kastvapen och nya ER-rörelser är
separata utökningar; nuvarande koboldkit kan leverera första versionen. Ruklar har bara sin
verifierade overhead som närstridsval och behåller net/Pierce Armor som befintliga specialfall.

**Spindelförbehåll:** originalets längre räckvidder väljer ofta eldattacker. Vår leap/latch/hop-
sekvens är redan en anpassning i projektet; vi bevarar den och lånar positioneringsbesluten.
Ingen spindel får eldattack bara för att originalets AI refererar till den.

## Alla varianter är tilldelade

`../ENEMY-AI-MAPPING.tsv` anger en roll för var och en av de 47 rosterposterna. Några viktiga skillnader:

- Vermin anfaller kort och lämnar plats åt större kobolds. Worker försvarar sin position.
  Miner/Tunneler gör korta verktygsanfall; Goldtooth söker en lucka. Ruklar utnyttjar nätet
  med overhead, Narg håller mark. Geomancer följer magikerrollen med koboldgruppen som skydd.
- Young/Mangy Wolf tar färre risker; Gray Forest Wolf håller trycket. Prowler får välja en
  närmare angreppspunkt eftersom dess nuvarande kit bara har det korta bettet.
- Thuros/Morgaine/Morgan använder befintlig bakifrånskada där den redan finns; Ambusher
  behåller sin eventstyrda entré. Jack/James är pressande närstridsmän utan påhittade sparkar
  om respektive kit saknar dem. Erlan får den långsamma yxrytmen.
- Princess/Porcine får gemensamma grupproller utan att skriva om DB-spawns. Mother Fang
  utnyttjar befintligt Web-fönster i sin språngplan, inte en andra parallell cooldown.

## Beslut och delade mekanismer

Familjepolicyn behöver avstånd/vinkel, sikt, spelarens **observerbara** rörelse/attackfas,
egen återhämtning, senaste utfall och allierades roller. Saknas en möjlig attack får den
positionera sig eller vänta. Den får aldrig välja ett rörelsenamn som inte finns i sitt kit.

Gruppsamordning sker per faktiskt möte och med ett gemensamt tak mot samtidiga anfall,
men lediga medlemmar får familjens uppgift: flank, håll mark, samla om, skydda ledaren eller
sök skottlinje. De får inte alla samma cirkelorder. En aktör får inte svälta i väntan på sin tur.

Säkra markpunkter och befintlig kollision styr varje förflyttning. En blockerad reträtt blir
väntan/närförsvar; en blockerad rusning får inte teleportera eller glida genom terrängen.
Återkomst efter leash, respawn, spelardöd och stagger tömmer respektive beteendes tillfälliga mål.

Beröringspunkter vid en godkänd implementation:
`species.rs` väljer beteende/variant, en separat beteendemodul tar besluten, `npc::think`
verkställer dem, `footwork.rs` levererar delad förflyttning där den passar. `pattern.rs`
och de befintliga kitten begränsar attackvalen. `named.rs`, `latch.rs` och `foebolt.rs`
behåller ansvar för specialförmågor, latch och projektiler.

## Bedömning i spelet

Först kontrasten **kobold/vildsvin/björn**, därefter övriga vanliga familjer och sist ledare/rares.
För varje roll: ensam fiende, tre allierade, spelaren stilla, närmar sig, backar, rullar,
slår och kastar; därefter trång terräng och blandade grupper. Samma seed och aktuell utrustning
ger jämförbara före/efter-filmer från spelkameran och framifrån.

Acceptans handlar om sekvenser, inte bara numeriska parametrar:

- Vildsvin förbereder/rusar/vänder och björn pressar/hugger/återhämtar; ingen väljer Circle.
- Spindel visar vänta/rikta/språng/latch/hoppa bort. Varg kan flankera; kobold gör korta utfall.
- Magiker försöker hålla kastavstånd men kan trängas. Bodyguards prioriterar sin ledare.
- Gruppbeteende skiljer sig mellan flockjakt, murlocrusning och eskort. Alla kan komma till anfall.
- Giltiga rörelser, träffögonblick, avbrott, leash och respawn fungerar. Nuvarande stridsvärden
  sparas för jämförelse; attackfrekvens kan påverka svårighet trots oförändrad skada och måste speltestas.

## Källor, beroenden och status

Lokala råskript finns endast i den ignorerade utvecklingscachen. Hashar och arkivreferenser
för de 37 granskade källorna finns i `../research/2026-10-16-enemy-ai-sources.tsv`.
Researchloggen är `../log/2026-10-16-enemy-family-ai-research.md`.

AI-regler och profiler som senare används av spelet ska ligga som vanliga projektfiler,
kunna granskas och versionshanteras, och fungera utan DSR-/ER-installation eller Lua-cache.
Referensfas, speltest, egna ersättningar och slutlig borttagning av härledda källberoenden
följer `../SOURCE-TO-ORIGINAL.md`. En översättning av källogik är fortfarande källbaserad.

Genomförande: `2026-10-16-enemy-family-ai.md`. Resultat: `../log/2026-10-17-enemy-family-ai.md`.
Animationer och Git-ignore är oförändrade. Inget är committat.
