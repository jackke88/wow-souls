# Familje-AI: undersökning och mappning

Jacob upplever att nästan alla fiender anfaller, går ut och cirklar som vargar. Uppgiften är
att undersöka DSR/ER och föreslå vilka beteenden som passar våra familjer, med Imp → kobold
som ett möjligt exempel.

## Levererat

- Design och familjetabell: `../plans/2026-10-16-enemy-family-ai-design.md`.
- Full roster: `../ENEMY-AI-MAPPING.tsv`, **47 unika poster, 15 föreslagna stridsroller**.
- Reproducerbara källreferenser: `../research/2026-10-16-enemy-ai-sources.tsv`, **37 skript**
  med spel, källarkiv, byteantal, SHA-256 och faktiskt avkodat GOAL-namn.
- Status är **förslag**, inte införd gameplay. Ingen spelkod, animation eller `.gitignore`
  ändrad. Hoggers separata omarbete lämnas åt den pågående uppgiften. Inget committat.

## Orsaken i nuvarande kod

`species.rs` ger WOLF-temperament åt boar/bear/forest_spider och THUG-temperament åt
gnoll/murloc/goblin. Härledda varianter ärver dem. `footwork::step` väljer samma
Close/Fresh/Circle/Hold/Back-sekvens och `npc::think` räknar samma två engagerade platser
inom tio yards för alla. Attackvillkoret kan avbryta betänketiden vid närhet.

`pattern.rs` beskriver sina attacktabeller som handskrivna ersättningar för originalets
Lua. Animationens källa och beteendets källa har alltså varit olika saker. Fler procentsatser
i `Temper` skulle fortfarande lämna samma övergripande sekvens.

## Lokal undersökning

Installationerna öppnades endast för läsning. DSR:s `script/*.luabnd.dcx` innehåller de
22 valda Battle-skripten, ibland identiskt återanvända i flera kartarkiv. I ER lästes BHD-
indexen och bara de 15 valda `/script/<id>_battle.luabnd.dcx` hämtades ur `Data0`.
Ingen full uppackning och ingen ändring av installationerna behövdes.

Extraktion: befintliga SoulsFormatsNEXT/UXM-läsare, i ett separat researchverktyg.
Avkodning: [DSLuaDecompiler](https://github.com/katalash/DSLuaDecompiler), commit
`c27340ab1b898a584748dc7e43a516cc1f1684f6`. Endast de två lokala projektfilerna i
researchkopian ändrades från net9.0 till net8.0 för datorns SDK. Själva dekompilatorlogiken
ändrades inte. Alla 37 filer gav exit 0; inga inbäddade DECOMPILER/ERROR/WARNING-markörer
hittades i Lua-resultaten. Detta är en skriptundersökning, inte ett test av originalens alla
grenar eller varje källmötes `NpcThinkParam`/eventkonfiguration.

Lokala instrument och rådata: `cache/ai-family-review/`:

- `extract/Program.cs`, `extract.log`, `scripts/inventory.tsv`.
- `decoded/dsr/`, `decoded/er/`, `decompile-status.tsv`.
- `features.tsv`: inventering av gemensamma GOAL-anrop, grupp-/hälso-/avstånds-/vinkelvillkor.
- `decompiler/`: egen avgränsad researchkopia; ingår inte i spelets bygge eller runtime.

## Granskade beslut som styr rekommendationen

| Referens | Verifierat i skriptet | Slutsats för vår mappning |
|---|---|---|
| ER Imp `308000` | `Activate` förgrenar på avstånd, vinkel, effekter, HP och villkorade Kankyaku/Torimaki-roller; Act10 lämnar målet, Act11 går åt sidan, Act13/14 tar säkerhets-/baksteg. | Bra liten opportunist. En full kopia har också cirklande/undanmanövrer; vår kobold behöver en egen kort rytm och befintligt kit som begränsning. |
| ER Imp `308010` | Separat variant med egna attackvikter och samma grundläggande grupp-/avståndsgrenar. | Referens för tyngre kobolds, inte en order att importera dess vapen eller animationer. |
| ER Wolf `407000` | Första bark/initialtillstånd, olika grupproller, front/bakvinklar, avstånd och `ApproachAround`/steg/retreater; vissa grenar gäller spirit-buddy. | Välj flockjaktens positioner. Använd vanliga fiendegrenar som referens; spirit-summon-specialfall hör inte till våra vargar. |
| DSR Black Dog `334000` | Avståndsviktade bett/utfall, separata steg/retreater efter anfall. | Jämförelse med vår nuvarande animationskälla; ER-vargens flockstruktur ger tydligare familjeidentitet. |
| DSR Hollow Thief `252000` | Korta attackkedjor, steg efter uppfattad attack/skada, guard- och projektilsvar. | Defias med lätta blad får egna offensiva vinklar och reaktioner med fördröjning. Guardgrenar kräver passande utrustning. |
| DSR Hollow Warrior/Soldier `254000`/`255000` | Avståndsviktade närstridsval; `255000` har dessutom uttryckliga svar på guard och flera angreppsföljder. | Grund för framåtriktade slagskämpar/gnolls och eskort, med rollskillnader och våra befintliga attacker. |
| DSR club-Hollow `253000` | Distans och spelarens guard ändrar vikterna; anfallen går via delade attackhjälpare. | Erlans tyngre, förberedda hugg får skilja sig från dolktjuvarnas rytm. |
| ER Demi-Human `410000`/`410020` | Beslut utifrån avstånd, allieradpositioner, villkorade grupproller, HP/effekter och vapenvariant. | Grund för murlocgruppens samlade rush/omgruppering. Den föreslagna samlingspunkten är vår anpassning, inte en påstådd exakt kopia av Lua. |
| DSR Armored Tusk `346000` | Riktningsberoende närförsvar, olika charge-avstånd, Approach/Attack/Wait samt kartbundna återvändningsgrenar. Efterattackfunktionen backstegar bara i en minoritet av fallen. | Rak anfallsplan/vändning; kopiera inte dess specifika kartregioner eller långa rusningssträckor. |
| ER Runebear `463000` | Avstånd, sida/bakvinkel, effekt-/hälsofall och många komboval; separat efterattack-GOAL. | Press och närförsvar med vår bite/swipe. Efterattack-GOAL:ens implementation har inte granskats; ingen exakt Runebear-replikering utlovas. |
| DSR Crag-Spider `340000` | Avståndsberoende angrepp, ApproachStep och bak-/sidosteg efter vissa attacker. Långa avstånd prioriterar attacker som vi inte använder. | Anpassa till vår befintliga språng/latch/hop-sekvens och behåll dess verkliga avstånd. Eldattacker följer inte med. |
| DSR Sorcerer `416000` | Distans-/timerviktade kast och nära attacker; efterattackfunktionen tar bak-/sidosteg vid närhot. | Äkta avståndshållande casterroll i stället för ärvd THUG-rörelse. |
| DSR Assassin `240000` | Backstab-observation, egna avståndsband, HP/timers och rörelse-/attackkedjor. | Garricks befintliga kit får en egen duellpolicy. |
| DSR Artorias `410000` | Eget boss-GOAL med attackkedjor och läges-/effektvillkor. | Matchar redan separat Hogger-arbete; ska inte ärva vanlig gnollpolicy. |

Övriga avkodade kandidater finns i källinventeringen. Bland jämförelserna: råttans `120000`
är enklare för korta svärmbett men saknar den humanoida gruppreferensen; Great Feline `536000`
har mycket steg undan och passar sämre som björnens rytm trots att vårt befintliga kit använder
dess bett. ER:s `301000`/`380000` och DSR:s `279000` ger för många vapen-/guard-/elitantaganden
för vanliga Defias. ER Perfumer `370000` kräver andra förmågor än våra befintliga bolt-casters.

Läsbara externa jämförelser:
[Imp](https://github.com/zoloypzuo/EldenRingAI/blob/master/308000_battle.dec.lua),
[Wolf](https://github.com/zoloypzuo/EldenRingAI/blob/master/407000_battle.dec.lua),
[Demi-Human](https://github.com/zoloypzuo/EldenRingAI/blob/master/410000_battle.dec.lua).
De lokala hashade skripten är underlaget för denna kartläggning; spegeln kan vara en annan version.

## Kontroll och nästa steg

Maskinell kontroll mot nuvarande `species::ROSTER`: 47 namn/konstanter matchar exakt,
inga dubletter eller saknade poster. Alla DSR-/ER-ID:n i mappningen finns i de 37 lokalt
verifierade skripten. Alla dokumentlänkar och TSV-kolumner kontrolleras före överlämning.
Ingen Rust-testsvit behöver köras för detta dokumentationspass; ingen runtimekod har ändrats.

Vid godkänt förslag: skriv implementationsplan med separata familjepolicyer, tester av
deras beslut och före/efter-filmer. Börja med kontrasten kobold/vildsvin/björn och fortsätt
hela tabellen. En föreslagen källa ska märkas referensbaserad tills den testats och ersatts
enligt `../SOURCE-TO-ORIGINAL.md`.
