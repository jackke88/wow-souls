> Superseded 2026-10-18 at the user's request: remove extra houses and vegetation omissions; keep sparse road traffic only. Current behavior: docs/GOLDSHIRE.md. The house survey is historical research and now writes only its research output, never production road data.

# Goldshire: bebodd by

Godkänt av Jacob 2026-10-17: rekommendationen om cirka 12–16 familjehus,
bestämda hem åt bofasta, lokala dagsrutiner och originalets småskaliga WoW-känsla.

## Resultat

- Utöka längs befintliga vägar med originalets beboeliga Elwynn-hus; olika
  riktningar, luckor, familjer och små gårdsdetaljer. Värdshus och smedja är centrum.
- Mät verklig husgeometri, mark under hela grunden, entré och vägen dit.
  Underkänn tomter med terräng genom golv, flytande grund, vatten, träd,
  befintliga objekt eller oframkomlig entré. Ändra inte originalinstallationen.
- Alla Goldshires bofasta får en bestämd bostad/familj. Bakgrundsinvånare har
  vardagsrutiner mellan hem, sysslor och mötesplatser med individuella tider.
  Questgivare och viktiga tjänster förblir tillgängliga. Utvalda civila kan resa.
- NPC-tillstånd fungerar över streaming och sparning. Närhet avgör fysisk
  simulering; klocka och ruttdistans beskriver livet längre bort.
- Inventera originalets flygande fåglar, smådjur, animerade miljöobjekt och ljud.
  Återanvänd originalresurserna, kontrollera rörelse och undvik att dubblera dem.

## Arkitektur

Ett projektägt Goldshire-tillägg håller bostäder, familjer, aktivitetsplatser,
rutter och extra miljöobjekt. Hus går genom samma WMO-strömning som originalet,
inklusive interiörer, möbler, belysning och kollision. Tillfälliga testvärldar
påverkas endast när tillägget uttryckligen aktiveras eller matchande data används.
Dagsschemat är lokal Rust-logik, utan språkmodell eller löpande tokenkostnad.

## Verifiering

Automatiska kontroller av familjemappning, rutters kontinuitet, dygnsskiften,
strömning och säker placering. Livekontroll av byn, entréer och NPC-förflyttning
vid flera klockslag; jämförelser i rörelse för fåglar och aktiviteter. Projektets
fmt, clippy och tester följs. Inga commits eller uppladdningar utan begäran.
