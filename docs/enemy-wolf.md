# Dark Souls Enemy Pattern — Wolf (Undead Attack Dog)

Motsvarighet i Dark Souls: **Undead Attack Dog [`c2260`]**.

Northshire Wolves (Young Timber Wolf / Timber Wolf) är snabba, aggressiva fyrbenta rovdjur. De förlitar sig på hög fart, språngattacker och oberäkneliga undanhopp för att lura spelaren att slå i luften.

---

## 1. Basattribut & Rörelse (Stats & Movement)

* **Hälsa (HP):** **45 HP** (Young Timber Wolf) / **65 HP** (Timber Wolf).
* **Poise (Balans):** **0 Poise**.
  * Avbryts omedelbart vid varje träff. Träff med 2H eller tung attack slår vargen till marken (`Knockdown / Rollback flinch`).
* **Stagger-varaktighet:** **0.45 s** (normal) / **1.20 s** (om den slås omkull).
* **Rörelsehastigheter:**
  * **Trot / Strafe (Cirklar spelaren):** **3.20 m/s** (~3.50 yd/s).
  * **Sprint Chase (Jaktspurt mot spelaren):** **6.20 m/s** (~6.78 yd/s).
    * *Extremt snabb!* Springer ikapp spelaren snabbt om denne inte sprintar själv.
* **Detektions- och AI-radie:**
  * **Aggro-radie:** **18.0 yards** (~16.5 m).
  * **Leash-gräns:** **28.0 yards** (~25.5 m).
  * **Cirkla-avstånd:** Håller sig ofta på 2.5–4.0 m avstånd och cirklar i sidled innan den exploderar i en språngattack.

---

## 2. Attack Patterns & Exakta Timings

### 2.1 Attack 1: Quick Snap Bite (Nypande snabbett)
*Ett snabbt bett framåt när spelaren befinner sig inom närstridsavstånd (< 1.5 m).*

* **Total längd:** **0.700 s** (42 frames @ 60fps / 21 frames @ 30fps)
* **Startup:** `0.000 s` till `0.233 s` (frames 0–13 @ 60fps / 0–6 @ 30fps) — *mycket snabb!*
* **Aktiv Hitbox:** `0.233 s` till `0.367 s` (frames 14–21 @ 60fps / 7–10 @ 30fps).
* **Recovery:** `0.367 s` till `0.700 s` (frames 22–41 @ 60fps / 11–20 @ 30fps).
* **Lunge:** **0.80 m** (~0.87 yd).
* **Skada:** **18 fysisk skada**.
* **Poise Damage:** **15 poäng**.
* **Block Stamina Damage:** **25 stamina drain**.

---

### 2.2 Attack 2: Leaping Pounce (Flygande språngbett)
*Vargens farligaste anfall. Triggas när avståndet är 2.5–5.5 meter.*

* **Total längd:** **1.267 s** (76 frames @ 60fps / 38 frames @ 30fps)
* **Telegraph & Hukning (Startup):**
  * Tid: `0.000 s` till `0.300 s` (frames 0–17 @ 60fps / 0–8 @ 30fps).
  * Vargen trycker ner bröstkorgen mot marken och spänner bakbenen.
  * **Tracking-fönster:** Låser anfallsvinkeln vid `0.200 s` (frame 12 @ 60fps).
* **Luftbana (Jumping Travel):**
  * Tid: `0.300 s` till `0.600 s` (frames 18–35 @ 60fps).
  * Kastar sig framåt hela **3.80 m** (~4.15 yd) genom luften med gapande käftar.
* **Aktiv Hitbox:**
  * Tid: `0.467 s` till `0.633 s` (frames 28–37 @ 60fps).
* **Landing & Recovery (Sårbar fas):**
  * Tid: `0.633 s` till `1.267 s` (frames 38–75 @ 60fps / 19–37 @ 30fps).
  * Vargen glider i marken och måste återfå fotfästet. Detta är spelarens bästa straff-fönster!
* **Skada:** **30 fysisk skada**.
* **Poise Damage:** **35 poäng** (bryter igenom normal brynja och stapplar spelaren).
* **Block Stamina Damage:** **45 stamina drain**.

---

### 2.3 Attack 3: Double Snap Combo (Dubbelbett i närstrid)
*Två på varandra följande bett (vänster käke följt av höger käke).*

* **Total längd:** **1.167 s** (70 frames @ 60fps / 35 frames @ 30fps)
* **Bett 1:**
  * Startup: `0.000 s` till `0.200 s` (frames 0–11 @ 60fps).
  * Aktiv Hitbox: `0.200 s` till `0.333 s` (frames 12–19 @ 60fps).
  * Skada: **14 skada**, **15 poise damage**.
* **Bett 2:**
  * Startup: `0.333 s` till `0.500 s` (frames 20–29 @ 60fps).
  * Aktiv Hitbox: `0.500 s` till `0.633 s` (frames 30–37 @ 60fps).
  * Skada: **16 skada**, **15 poise damage**.
* **Recovery:** `0.633 s` till `1.167 s` (frames 38–69 @ 60fps).

---

## 3. Försvarsmanöver: Evasive Hop (Undanhopp)

Precis som i Dark Souls har vargen en defensiv undanmanöver:

* **Aktivering:** Om spelaren närmar sig med dragen klinga eller inleder en sving inom 2.0 meter har vargen 40% chans att utföra ett snabbt undanskutt.
* **Varaktighet:** **0.500 s** (30 frames @ 60fps / 15 frames @ 30fps).
* **Förflyttning:** Skuttar **1.80–2.20 m** i sidled (eller snett bakåt).
* **Cooldown på undanhopp:** Minst **2.5 sekunder** innan nytt skutt kan ske.
* **Syfte:** Lurar spelaren att svinga i tomma intet, varpå vargen omedelbart följer upp med ett `Quick Snap Bite` eller `Leaping Pounce`.
