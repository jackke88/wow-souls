# Liv på Goldshires vägar

Sex nya civila går längs originalets kullerstensvägar och tar korta pauser vid
sina hållplatser. Fyra har lokala rutter kring Goldshire, två går mellan byn och
Stormwinds infart. Rutten slutar före stadens mur; stadens interiörer ingår inte.
Varje person har egen startfas och paustid. De använder originalets mänskliga
modeller, gånganimationer och enstaka samtalsgester.

Husförslaget från 17 oktober ersattes på Jacobs begäran 18 oktober. Inga nya hus,
vegetationsundantag eller andra miljöplaceringar laddas. Alla originalinvånare,
questgivare, tjänster, patruller, fåglar, fjärilar och smådjur behålls.

Originalvägarna finns i `data/world/goldshire-roads.tsv` (format `wsroads 1`),
resenärernas identiteter och hållplatser i `village/scene.rs`. GUID 9100000–9100005
och entry 9200000–9200005 är stabila för senare innehåll. Resenärerna har inga
quest- eller handelsflaggor i den här versionen. Hemliga möten, quests och varor
kan läggas till på dessa personer senare.

Synliga civila följer varje väghörn med den vanliga kollisionsmotorn. Borta från
spelaren beräknas deras plats ur den sparade klockan, inklusive hela dagar.
Dygnsskifte återställer inte rutten. Paus och fryst klocka stoppar gångschemat.
Allt körs lokalt, utan språkmodeller eller löpande tokenkostnad.

`WOWSOULS_VILLAGE=0` stänger av trafiken. `WOWSOULS_VILLAGE_TRACE=1` loggar
rutter och framsteg. För rörelsekontroll används en löpande klocka, exempelvis
`WOWSOULS_DAY_MINUTES=24`; `WOW_CAPTURE` ska vara av eftersom det fryser
originalets miljöanimationer.

Kontroller med originalinstallationen tillgänglig:

```powershell
cargo test -j 4 -p wow-world village --offline
cargo test -j 4 -p wow-souls village:: --offline -- --nocapture
```

Det fysiska vägtestet använder originalets ADT-mark, WMO-byggnader, M2-hinder och
möbler. Det provar alla sex använda rutter i båda riktningarna med spelarens
kapsel och vanliga gångmotor, utan hopp eller teleportering.