# DSR och Elden Ring: från referens till eget material

**Stående beslut från Jacob, 2026-10-16.** Detta är projektets gemensamma arbetsflöde för
Dark Souls Remastered (DSR) och Elden Ring. Följ det vid nya animationer, movesets och andra
delar som bygger på dessa spel.

## Mål och nuläge

Vi utgår först från DSR och Elden Ring för att få in fungerande rörelser och stridsbeteenden,
testar dem i vårt spel och bygger därefter egna ersättningar. **Slutmålet är noll beroende
till DSR och Elden Ring**, både vid bygge, installation och körning av slutversionen.

Båda spelen finns nu lokalt på utvecklingsdatorn. DSR har en befintlig läsare/importkedja.
Elden Ring har nu en verifierad provimport av löpning, sprint och rullning: redigerbara klipp i
`anim/candidates/elden-ring/`, jämförda i spelet. Standardrörelserna är fortfarande DSR.
Se [ER-jämförelsen](log/2026-10-16-elden-ring-motion-review.md). Nya ER-familjer måste verifieras var för sig.
DSR är förstaval där vi redan har fungerande material. Elden Ring används när det passar
behovet bättre eller när DSR saknar en tillräckligt bra rörelse.

Nuvarande lokala källplatser (läsning utan ändringar):

| Spel | Plats på utvecklingsdatorn |
|---|---|
| DSR | `C:\Spel\Dark Souls Remastered` |
| Elden Ring | `C:\Spel\Elden.Ring.REPACK-KaOs` |

Detta är maskinens nuvarande platser. Kod och installationsflöden ska använda konfigurerade
sökvägar, så att utvecklingen och bygget inte låses till dessa kataloger.

Nollberoendet gäller dessa två spel. Projektets användning av WoW-data hanteras separat.
Detta dokument beskriver arbetsordningen och slutkravet; dagens prototyp använder fortfarande
DSR-material och är ännu inte oberoende.

## Teknisk frikoppling 2026-10-17

Spelbygget och körningen kräver nu bara WoW 1.12.1 som spelinstallation. Motorkoden ligger i
`vendor/benilla`, den aktiva världen och rösterna i `data`, och animationerna i `anim/source`.
DSR-verktygen är separata utvecklingsarbetsytor. Spelvägen läser varken DSR-/ER-installationer,
extraktionsmanifest, binära klippcacher eller DSR-HUD-bilder, även om gamla miljövariabler är satta.
Detta uppfyller installationsfrikopplingen. Kurvorna och importerade parametrar har bevarats och
är fortfarande källbaserade; statusen är därför fortsatt `SOURCE_TESTED`, inte `SOURCE_REMOVED`.
Den oberoende ersättningsfasen nedan återstår. Kontroll: `tools/Test-WowOnly.ps1`.

## Arbetsordning för varje rörelsefamilj eller funktion

1. **Välj och dokumentera källan.** Ange spel, fil/klipp/ID, varför det passar, vilka data som
   används och vilka avvikelser som behövs. Läs installationerna utan att ändra dem. Extraherade
   speldata och binära extraktionscacher hålls i ignorerade utvecklingscacher. **Jacobs uppdaterade
   beslut 2026-10-16:** de redigerbara animationsfilerna och deras uppspelningstabeller är vanliga
   projektfiler i `anim/source/` och ska kunna versionshanteras i Git. De ignoreras inte.
2. **Integrera källversionen först.** Få animation, retargeting, root motion, vapen-/stavgrepp,
   träff- eller projektilögonblick, övergångar och gameplay att fungera tillsammans i spelet.
   Markera materialet som en tillfällig källbaserad implementation.
3. **Testa den fungerande helheten.** Bedöm rörelsen i spelkameran och framifrån i täta
   bildserier eller video. Kontrollera kropp/proportioner, fötter, räckvidd, läsbarhet, timing,
   avbrott och faktisk träff/projektilrelease. Spara resultat och vad som ska bevaras i den egna
   versionen. En lyckad extraktion eller en snygg stillbild räcker inte som acceptans.
4. **Skapa vår egen ersättning.** Använd den testade spelkänslan som krav och skapa egna
   animationskurvor, poser, root motion och gameplayhändelser. Ta fram egna parameter- och
   balansvärden samt egna tillgångar för de delar som används. En retargetad, omtimad eller
   procedurmässigt remixad källanimation innehåller fortfarande källdata; den är inte färdig
   som en oberoende ersättning bara för att filnamn eller format har ändrats.
5. **Integrera och testa ersättningen.** Byt den aktiva spelvägen till vårt material och testa
   samma situationer igen. Egna rörelser och värden får utvecklas vidare; dokumentera och
   verifiera ändringar av spelbeteendet. Källklipp får inte krävas som dold fallback.
6. **Avveckla källberoendet för delen.** Ta bort dess behov av originalfiler, extraherade klipp,
   genererade källcacher och importerade tabeller i produktionsvägen. Uppdatera startflöde,
   konfiguration, bygge, paketering, tester och instruktioner som tidigare krävde källspelet.
   Verifiera detta innan delen markeras som fri från källberoenden.

För större kit kan arbetet göras i etapper. Planera ersättningen när källdelen införs, och
för den vidare efter integrationstestet. Ett godkänt källbaserat kit är en fungerande prototyp
och ett underlag för nästa etapp; hela kedjan är klar först efter steg 6.

## Spårning och ansvar

Den som inför eller ersätter en del uppdaterar dess plan/arbetslogg med källa, aktuellt
beroende, testbevis, egen ersättning och nästa steg. Använd dessa statusar:

| Status | Vad som är verifierat |
|---|---|
| `SOURCE_INTEGRATING` | Källmaterial valt; integration pågår. |
| `SOURCE_TESTED` | Källversionen fungerar och har testats i spelet. |
| `ORIGINAL_IN_PROGRESS` | Vår ersättning skapas och kopplas in. |
| `ORIGINAL_TESTED` | Vår ersättning fungerar och har testats i spelet. |
| `SOURCE_REMOVED` | Produktionsvägen fungerar med källspelet och dess härledda data otillgängliga. |

Behåll historiska källhänvisningar och uppgifter om tredjepartskod när material ersätts.
Aktuell status ska visa vad den aktiva versionen faktiskt använder.

Det redigerbara animationsbiblioteket kan läsas utan DSR-installationen eller den binära
extraktionscachen. Detta avlägsnar installationskravet för klippen. Kurvorna är fortfarande
källbaserade tills egna ersättningar har skapats och verifierats; filflytten innebär därför inte
status `SOURCE_REMOVED`. Se [animationsbiblioteket](../anim/README.md).

## Slutkrav för en version utan DSR-/ER-beroenden

Kontrollen omfattar allt som används från dessa spel: animationer och skelettdata,
root motion, event-/timingdata, parametertabeller, UI, typsnitt, bilder, ljud och effekter.
Även extraherat material i eget filformat eller en genererad cache ingår i inventeringen.

Verifiera slutversionen i en isolerad miljö där varken DSR-/ER-installationer eller deras
extraherade/härledda cacher finns tillgängliga. Bygg och paketera från våra egna källor och
tillgångar, installera och kör hela spelets flöde, inklusive samtliga klasser, vapen och fiender.
Kontrollera att ingen produktionsväg försöker läsa källspelen, kräver deras sökvägar, startar
en extraktor eller återskapar källdata vid första körningen. Produktionskontroller ska kunna
köras med egna testdata och får inte dölja ett beroende genom att hoppa över tester.

Läsare och undersökningsverktyg får finnas kvar som separata utvecklingsverktyg. De ska vara
frikopplade från slutversionens bygge, paket, start och körning. Spelaren ska varken behöva
installera eller tillhandahålla DSR eller Elden Ring för slutversionen. Det kravet är uppfyllt
först när alla använda delar har egna verifierade ersättningar och den isolerade kontrollen passerar.

Teknisk källhistorik och kodlicenser: [PROVENANCE.md](PROVENANCE.md).
Aktuella vapenimplementationer: [WARRIOR-WEAPONS.md](WARRIOR-WEAPONS.md).
Klippkatalog: [animations.md](animations.md).
