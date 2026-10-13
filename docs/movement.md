# Dark Souls Remastered — Movement & Locomotion Mechanics

Exakta värden, hastigheter, accelerationer och stamina-förbrukning för spelarrörelse i Dark Souls Remastered (DSR), formaterade för implementering i `wow-souls`.

---

## 1. Hastighetsnivåer (Locomotion Speeds)

Dark Souls har tre distinkta markhastigheter: **Walk**, **Run (standard jogg)** och **Sprint (dash)**.

| Tillstånd | Hastighet (m/s) | Hastighet (WoW yd/s) | Relativ faktor | Input (Tangentbord / Gamepad) | Stamina-kostnad |
|---|---|---|---|---|---|
| **Walk** | **2.18 m/s** | **2.38 yd/s** | 1.0× | `Ctrl` hålls nedtryckt / Spak lätt lutad (< 0.5) | 0 (Regenererar stamina) |
| **Run** | **4.36 m/s** | **4.77 yd/s** | 2.0× | `WASD` / Full spak (> 0.5) | 0 (Regenererar stamina) |
| **Sprint** | **6.54 m/s** | **7.15 yd/s** | 3.0× | Håll `Space` / `B` intryckt under Run | **21.0 poäng / sekund** |

> [!NOTE]
> I WoW 1.12 standard är bashastigheten för löpning $7.0\text{ yd/s}$ och gång $2.5\text{ yd/s}$.
> DSR:s sprint ligger på $\approx 7.15\text{ yd/s}$, vilket gör att Dark Souls Run ($4.77\text{ yd/s}$) känns mer jordnära, tung och taktisk jämfört med WoWs snabba standardspring.

---

## 2. Sprint & Stamina-regler

1. **Sprint-initiering:**
   * Kräver minst **1 poäng stamina**.
   * Spelaren måste redan röra sig i minst Run-hastighet. Om `Space` hålls nedtryckt från stillastående sker ett Backstep så fort knappen släpps.
2. **Stamina Drain:**
   * Takt: **21.0 stamina per sekund** (`0.35 poäng per frame @ 60fps`).
   * Om stamina når `0` upphör sprint omedelbart och spelaren tvingas ned till Run.
3. **Regenerations-fördröjning:**
   * När spelaren släpper sprint dröjer det **0.60 sekunder** (36 frames @ 60fps) innan stamina börjar ticka upp igen.

---

## 3. Inertia, Svängradie och Skids (Turn Acceleration)

Dark Souls har fysisk tröghet som förhindrar ögonblicklig 180°-vändning utan kostnad:

* **Linjär acceleration:**
  * Från stillastående till Run: **0.15 s** (~9 frames @ 60fps) för att nå full topphastighet (mjuk S-kurva).
  * Inbromsning till stopp: **0.20 s** glidsträcka (~0.45 m / 0.5 yd).
* **180° Skid (U-sväng):**
  * Om spelaren under full Run eller Sprint hastigt trycker i motsatt riktning (vinkel $> 135^\circ$) triggas en **turn-around skid**:
    * Varaktighet: **0.30–0.35 s** (18–21 frames @ 60fps).
    * Karaktären bromsar in hårt mot marken och kan inte initiera attacker under de första 0.20 sekunderna.

---

## 4. Hopp-mekanik (Running Jump)

I Dark Souls 1 / DSR är hopp strikt kopplat till sprint och är inte ett fritt "bunny hop":

* **Aktivering:** Kan endast utföras medan karaktären befinner sig i **Sprint** (hastighet $\ge 6.0\text{ m/s}$).
  * Tangent: Tryck `Space` (eller `L3`/stick-click i DSR) under pågående sprint.
* **Hopp-faser och varaktighet:**
  * **Takeoff (Wind-up):** 0.10 s (6 frames @ 60fps). Fötterna lämnar marken.
  * **Air time (I luften):** **0.55 s** (33 frames @ 60fps).
  * **Landing Recovery:** **0.25 s** (15 frames @ 60fps). Karaktären absorberar landningen i knäna innan full kontroll återfås.
  * **Total animationstid:** **0.90 s** (54 frames @ 60fps).
* **Horisontell räckvidd:** ~4.5–5.0 m (~5.0–5.5 yd) beroende på sprintens ingångshastighet.
* **Hopphöjd (Vertikal bana):** ~0.85 m (~0.93 yd) maximal apex.
* **Stamina-kostnad:** 0 extra stamina (utöver vad sprinten kostade fram till avstampet).
* **i-frames:** **0 frames**. Hoppet ger inga i-frames och träffas karaktären i luften resulterar det i en knockdown/stagger.

---

## 5. Mållåsning (Locked-on Strafe Speeds)

När karaktären har ett mål låst (`Lock-On`) hålls ansiktet riktat mot målet och rörelsen övergår till cirkulär strafe och backpedal:

| Riktning relativt målet | Hastighet (m/s) | Hastighet (WoW yd/s) | Notering |
|---|---|---|---|
| **Framåt (Mot målet)** | **4.36 m/s** | **4.77 yd/s** | Standard run-hastighet rakt mot fienden. |
| **Strafe (Vänster / Höger)** | **3.85 m/s** | **4.21 yd/s** | ~88% av normal fart. Cirklar målet med jämn radie. |
| **Backpedal (Bakåt)** | **3.05 m/s** | **3.34 yd/s** | ~70% av normal fart. Backar bort med blicken fäst på målet. |
| **Sprint under Lock-On** | **6.54 m/s** | **7.15 yd/s** | Bryter strafe-hållningen och springer fritt i kamerans riktning. |
