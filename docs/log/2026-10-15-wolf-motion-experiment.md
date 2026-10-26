# Vargar: försök med redigerbara rörelser, 2026-10-15

Användarens uppdrag: prova DSR:s rörelsesiffror som utgångspunkt, behåll Souls-känslan, gör en egen variant och dokumentera arbetsinsats och tokenåtgång.

## Vad försöket gör

`WOWSOULS_WOLF_MOTION=1` väljer en experimentell variant. Utan inställningen används dagens animationer. Implementation: `crates/wow-souls/src/wolf_motion.rs`, ansluten i `monster::load`.

Två små recept i Rust styr attackens faser och sju nyckelposer med vinklar i grader: nacke, huvud, käke och svans. Vi återanvänder den befintliga kroppens och benens rörelse, men ger dem en ny tidskurva. Samma klocka används för kropp, förflyttning, träfffönster och händelser. Klockan är monoton och använder mjuka hastighetsövergångar; posernas extra rotationer går mjukt in och ut. Första och sista posen samt den sammanlagda förflyttningen bevaras.

- **Bett:** lägre nacke, huvudet åt sidan under uppladdningen, ett snabbt bett över mittlinjen, därefter huvudrekyl och en motsatt svansrörelse.
- **Utfall:** längre förvarning, sänkt nos före avstampet, snabbare attackfas och tydligare huvudrörelse efter landning.
- Vanliga Wolf-modellen får båda attackerna. Young Wolf, Timber Wolf, Mangy Wolf och Gray Forest Wolf delar denna rigg. Prowlers separata DireWolf-rigg får bara bettet; dess tidigare underkända utfall har inte återinförts.

## Uppmätta skillnader

| Rörelse | Ursprunglig längd | Ny längd | Ursprungligt träfffönster | Nytt träfffönster | Huvudets extra rotation vid uppladdning |
|---|---:|---:|---:|---:|---:|
| Wolf bett, `c3340/a00_3000` | 1,667 s | 1,500 s | 0,633–0,867 s | 0,580–0,740 s | 19,57° |
| Wolf utfall, `c3340/a00_3004` | 1,500 s | 1,600 s | 0,333–0,700 s | 0,460–0,700 s | 12,17° |
| DireWolf bett, `c3340/a00_3000` | 1,667 s | 1,500 s | 0,633–0,867 s | 0,580–0,740 s | 19,57° |

Extra tassavvikelse vid motsvarande källtid: högst **0,00000038 modellenheter** (avrundningsfel, före entitetsskala). Benens lokala positioner och skalor är bevarade; inga kroppsförlängningar används. Slutlig framåtförflyttning är fortfarande 0,828336 / 2,643942 / 0,766698 modellenheter, före respektive vargs entitetsskala.

Skada, poise, basradier och AI:s avståndsband är oförändrade. Huvudets nya rörelse flyttar käkens faktiska träffvolym något; därför ingår riktiga strider i kontrollen. Ändrade träfffönster betyder att spelbalansen ändå förändras.

## Ursprung och begränsning

Detta är en **bearbetning av DSR-data**. Källans kroppsrörelse finns kvar och receptet läggs ovanpå den. Det är varken en självständigt skapad animationsuppsättning eller ett påstående att vi nu äger DSR-animationerna. Käll-ID:n och ursprung sparas öppet. Originalcachen och referensskelettet skrivs inte om; jämförelsematerial och exporter stannar i gitignorerade `cache/wolf-motion/`.

Försöket tar inte bort spelets DSR-beroende. Nästa större steg vore egna helkroppsposer och en separat passning av tasskontakter/IK. Även gång, reaktioner, död och övrig DSR-data återstår vid en full ersättning.

## Verifiering och visuell bedömning

- Formatering enligt handover passerar. Workspace Clippy med `-D warnings` passerar.
- **1 268 tester passerar** med `DSR_DIR` och `WOW_DATA` satta: wow-souls 880, wow-world 240+4, dsr-cache 88, dsr-formats 52, wscl 4.
- Två nya integrationstester kontrollerar verkliga klipp: korrekt event-/förflyttningsklocka, ordnad tid, normaliserade rotationer, första/sista pose, alla tassars modellpositioner, oförändrade lokala benpositioner/skalor/skador samt att ett felaktigt källklipp lämnar hela kitet orört. De skriver tydligt SKIP när lokal cache saknas; här kördes de mot de riktiga tre klippen.
- 39 ursprungliga cache-/referensfiler är SHA-256-kontrollerade och oförändrade. Ingen extraktion eller skrivning till spelinstallationerna gjordes.
- Täta bildserier granskades för bett och utfall, från sidan och från vargens främre sida. Bettet har en synlig sidovänd uppladdning, snabb passage över mittlinjen och huvudrekyl. Utfallet har längre läsbar förvarning och en tydligare landningsgest. Inga nya utdragna ben, kroppsförlängningar eller abrupta skalbyten observerades.
- Riktiga strider med spelkameran bekräftar kontakt i det nya träfffönstret: bett vid **0,58 s**, utfall vid **0,65 s** i den kontrollerade scenen. Originalet träffar vid 0,63 respektive 0,33 s. Samma oförändrade skada efter rustning: 17 för bett och 16 för utfall i dessa testfigurer.
- Det första bettprovet startade längre bort och missade även i originalet. Ett nytt prov startar vid 1,1 yd; båda varianterna träffar då. Det befintliga förhållandet mellan bettets AI-avståndsband och faktisk käkräckvidd är fortfarande ett balanseringsärende. Försöket ändrar inga avståndsband för att dölja det.
- Kamerorna justerades efter de första inspelningarna för att få med hela vargen och se dess ansikte. Ett fel i inspelningsskriptets hantering av en ensam PowerShell-array rättades; felmärkta provklipp används inte i slutjämförelsen.

Jämförelsefilm: `cache/wolf-motion/comparison.mp4` (original till vänster, remix till höger). Den visar två klipp från sidan, två framifrån, bett/utfall med spelkameran samt strid framifrån och Prowler. I klippvyn är guldskelettet den oförändrade DSR-referensen vid samma förflutna tid; efter vår tidsändring är det **inte** en referens för identisk rörelsefas.

**Bedömning:** metoden är användbar för snabb iteration och en tydlig remix. Souls-känslan sitter till stor del i förvarning, acceleration, bindningen till attacken och återhämtningen. Skillnaden i huvud/käke är tydlig i närbild och mindre från den vanliga spelkameran. Kroppens huvudrörelse är fortfarande igenkännbar från källan; försöket når inte en helt egen varganimation. En sådan kräver egna helkroppsposer och ett särskilt arbete med tasskontakter.

Återupprepa lokalt:

```powershell
# Från repots rot, med de två befintliga installvariablerna satta:
cargo test -j 4 --offline -p wow-souls wolf_motion -- --nocapture
cargo build -j 4 --offline -p wow-souls
& ./cache/wolf-motion/capture.ps1 -Mode loops
& ./cache/wolf-motion/capture.ps1 -Mode fight
& ./cache/wolf-motion/capture.ps1 -Mode prowler
& ./cache/wolf-motion/compare.ps1
```

`WOWSOULS_WOLF_EXPORT=<lokal katalog>` vid testkörningen exporterar försöksklippen för numerisk inspektion. Det behövs inte för normal spelkörning.

## Arbetsinsats och tokens

**Tidsåtgång:** 21:33:59–22:00:26 svensk tid, **26,4 minuter** inklusive läsning, plan, implementation, byggen, tester, omtagningar av kameror/testscener, videogranskning och dokumentation. Ingen mänsklig arbetstid eller framtida produktionsprognos mäts här.

Kodens omfattning: **472 rader** i den nya modulen, varav **325** före testdelen, plus en liten inkoppling i monsterladdaren. Inspelnings- och jämförelseskript ligger lokalt under cache/wolf-motion. Inga extra modeller/subagenter eller bildgenerering användes.

Faktisk tokenanvändning i denna chatt, differens från posten före uppdraget:

| Typ | Tokens |
|---|---:|
| Input utan cache | 225 881 |
| Output inklusive reasoning | 54 475 |
| Cachad input | 8 177 792 |
| **Totalt input + output** | **8 458 148** |

Input utan cache plus output: **280 356** tokens. Av output är 25 621 reasoning-tokens; de ingår redan i output och ska inte adderas igen. Den stora inputmängden omfattar återläsning av samma projekt-/verktygskontext under många anrop, huvudsakligen via cache. Detta är loggad tokenanvändning, inte ett debiterat belopp eller antalet nya ord i koden.

Mätpunkt: 2026-10-15T19:59:42.531Z, före rapportens sista skrivning och slutmeddelandet. Baslinje och maskinläsbara siffror finns i cache/wolf-motion/usage-baseline.json respektive usage-final.json. Slutmeddelandet ingår inte i mätningen.

Arbetet omfattar en återanvändbar fas-/posekärna, två recept med sammanlagt 14 nyckelposer, stöd för två olika riggar, integration och två tester, deterministiska inspelningsscener, jämförelsefilm och dokumentation. Den första kärnan är huvuddelen av kodarbetet. Därefter kräver en ändrad huvudvinkel eller tid bara en ändring i receptet; större ändringar av kroppen och kontakterna kräver mer animationsarbete. Originalversionen ligger kvar och är standard. Ingenting i detta försök har publicerats.

Inga commits har gjorts. Befintliga, ocommittade ändringar i andra delar av spelet har lämnats kvar.
