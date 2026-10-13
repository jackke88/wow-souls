# Dark Souls Enemy Pattern — Defias Thief (Undead Assassin)

Motsvarighet i Dark Souls: **Undead Assassin / Hollow Thief [`c2200`]** (från Lower Undead Burg).

Northshire Defias (Defias Thug / Defias Bandit / Thief) är lönnmördare och banditer klädda i läder och röda huvor/bandanas. De strider med dolk och liten parersköld (Buckler/Target Shield). De är blixtsnabba, använder kastknivar på distans, bygger upp blödning (*Bleed*) och kan parera och riposta spelarens förutsägbara anfall.

---

## 1. Basattribut & Rörelse (Stats & Movement)

* **Hälsa (HP):** **90 HP** (Defias Thug) / **125 HP** (Defias Bandit / Thief).
* **Poise (Balans):** **15 Poise**.
  * Tål en pil eller en svag rolling attack utan att avbrytas, men ett direkt 1H Longsword R1 (som gör 20 Poise Damage) bryter tjuven i en `Flinch / Stagger`.
* **Stagger-varaktighet:** **0.50 s** (30 frames @ 60fps).
* **Rörelsehastigheter:**
  * **Combat Strafe (Cirklar aktivt mot spelarens rygg):** **4.10 m/s** (~4.48 yd/s) — *mycket snabb strafe för att söka backstab!*
  * **Sprint Chase (Jakt):** **5.80 m/s** (~6.34 yd/s).
* **Detektions- och AI-beteende:**
  * **Aggro-radie:** **18.0 yards** (~16.5 m).
  * **Ambush-beteende:** Står ofta dold bakom träd, tält eller klippblock och gör ett överraskningsutfall när spelaren passerar.
  * **Flanking AI:** Rör sig inte i en rak linje mot spelaren, utan cirklar åt vänster/höger för att försöka komma bakom spelarens sköld.

---

## 2. Attack Patterns & Exakta Timings

### 2.1 Attack 1: Rapid 3-Hit Dagger Flurry (3-slags dolk-kombination)
*En serie snabba hugg med dolken. Orsakar fysisk skada och bygger upp Bleed.*

* **Total längd (hela trippeln):** **1.500 s** (90 frames @ 60fps / 45 frames @ 30fps)
* **Slag 1 (Höger -> Vänster snedhugg):**
  * Startup: `0.000 s` till `0.233 s` (frames 0–13 @ 60fps) — *blixtsnabb!*
  * Aktiv Hitbox: `0.233 s` till `0.333 s` (frames 14–19 @ 60fps).
  * Skada: **14 skada**, **10 poise damage**, **+35 Bleed buildup**.
* **Slag 2 (Vänster -> Höger mot-hugg):**
  * Startup: `0.333 s` till `0.467 s` (frames 20–27 @ 60fps).
  * Aktiv Hitbox: `0.467 s` till `0.567 s` (frames 28–33 @ 60fps).
  * Skada: **14 skada**, **10 poise damage**, **+35 Bleed buildup**.
* **Slag 3 (Avslutande djupt stick):**
  * Startup: `0.567 s` till `0.767 s` (frames 34–45 @ 60fps).
  * Aktiv Hitbox: `0.767 s` till `0.900 s` (frames 46–53 @ 60fps).
  * Skada: **22 skada**, **20 poise damage**, **+40 Bleed buildup**.
* **Recovery (efter sista slaget):**
  * Tid: `0.900 s` till `1.500 s` (frames 54–89 @ 60fps).
* **Lunge (Glid framåt):** Rör sig totalt **1.80 m** framåt under hela kombinationen.

> [!WARNING]
> **Bleed Mekanik:** Spelaren har en Bleed-mätare på 100 poäng. Om alla tre slagen träffar fylls mätaren ($35 + 35 + 40 = 110$) och spelaren drabbas av ett massivt blödningsstraff som omedelbart drar **30% av spelarens totala max-HP**!

---

### 2.2 Attack 2: Leaping Ambush (Hoppande dolk-stöt)
*Triggas när spelaren håller ett avstånd på 3.5–6.0 meter.*

* **Total längd:** **1.300 s** (78 frames @ 60fps / 39 frames @ 30fps)
* **Startup:** `0.000 s` till `0.267 s` (frames 0–15 @ 60fps).
* **Luftbana:** `0.267 s` till `0.567 s` (frames 16–33 @ 60fps).
  * Kastar sig framåt **3.50 m** (~3.83 yd) med båda armarna framåt.
* **Aktiv Hitbox:** `0.567 s` till `0.700 s` (frames 34–41 @ 60fps).
* **Recovery:** `0.700 s` till `1.300 s` (frames 42–77 @ 60fps).
* **Skada:** **32 fysisk skada**, **30 poise damage**.

---

### 2.3 Attack 3: Throwing Knife (Kastkniv på distans)
*Om spelaren backar undan eller dricker potion på 6.0–15.0 meters avstånd kastar tjuven en kastkniv.*

* **Total längd:** **0.900 s** (54 frames @ 60fps / 27 frames @ 30fps)
* **Wind-up (Drar kniv ur bältet):** `0.000 s` till `0.367 s` (frames 0–21 @ 60fps).
* **Projektil avfyras (Spawn frame):** Frame 22 (`0.367 s`).
  * **Projektilhastighet:** **24.0 m/s** (~26.2 yd/s) i rak linje mot spelaren.
* **Recovery:** `0.367 s` till `0.900 s` (frames 22–53 @ 60fps).
* **Skada:** **16 fysisk skada**, **+25 Bleed buildup**. Kan blockeras med sköld (kostar bara 10 stamina att blocka).

---

## 3. Defensiva Specialmekaniker

### 3.1 Parry & Riposte (Parering med Target Shield)
*Det mest ökända draget hos Undead Assassin i Dark Souls 1!*

* **Aktivering:** När spelaren står rakt framför tjuven har denne 20% chans att höja sin lilla buckler i en pareringsställning istället för att attackera.
* **Parry-fönster (Aktiv parering):**
  * Varar i **0.200 s** (12 frames @ 60fps).
* **Om spelaren svingar och träffar skölden under detta fönster:**
  1. Spelarens sving pareras bort med ett metalliskt klangljud.
  2. Spelaren hamnar i `Parried Stagger` och fryses i **1.40 s** (kan inte röra sig eller blocka).
  3. Tjuven kliver in och gör en **Riposte (Kritisk stöt)**:
     * Skada: **80 fysisk skada** (ofta dödligt för en skadad karaktär!).
* **Motmedel för spelaren:** Svinga inte vilt framifrån mot en vaksam Defias Thief; använd spark (`Kick / Guard Break`), rulla bakom för backstab, eller attackera under tjuvens återhämtningsfas!

### 3.2 Evasive Quickstep (Bakåtvolt / Undansmiter)
* Om spelaren pressar tjuven med flera slag har tjuven ett snabbt undanspring bakåt:
* **Varaktighet:** **0.467 s** (28 frames @ 60fps).
* **Förflyttning:** Hoppar bakåt **2.20 m** (~2.40 yd) för att nollställa avståndet.
