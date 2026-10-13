# Dark Souls Enemy Pattern — Kobold (Undead Hollow Miner)

Motsvarighet i Dark Souls: **Undead Hollow (Club / Pickaxe / Miner) [`c2100`] / Infested Ghoul [`c2210`]**.

Northshire Kobolds (Kobold Worker / Kobold Vermin) är kutryggiga, beväpnade med enkla gruvverktyg/påkar, fysiskt svaga men oberäkneliga och farliga i grupp.

---

## 1. Basattribut & Rörelse (Stats & Movement)

* **Hälsa (HP):** **60 HP** (Northshire Vermin) / **80 HP** (Worker).
* **Poise (Balans):** **0 Poise**.
  * Avbryts (`Flinch / Stagger`) av alla spelarattacker (även 1H lätt attack).
* **Stagger-varaktighet:** **0.50 s** (30 frames @ 60fps) vid träff.
* **Rörelsehastigheter:**
  * **Walk (Patrull / Retur hem):** **2.20 m/s** (~2.40 yd/s).
  * **Chase (Jakthastighet mot spelaren):** **3.80 m/s** (~4.15 yd/s) — något långsammare än spelarens run (4.36 m/s), vilket låter spelaren skapa avstånd.
* **Detektions- och AI-radie:**
  * **Aggro-radie:** **16.0–20.0 yards** (~15–18 m).
  * **Leash-gräns (Ger upp och går hem):** **30.0 yards** (~27.5 m).
  * **Attack-räckvidd (Trigger):** Inom **1.80 yards** (~1.65 m) och vinkel $|\cos\theta| \ge 0.80$.
  * **Mellanslags-cooldown:** **1.20–1.50 s** mellan genomförda attacksekvenser.

---

## 2. Attack Patterns & Exakta Timings

### 2.1 Attack 1: Telegraphed Overhead Slam (Enkelt hårt nerslag)
*Klassisk telegraferad attack: lyfter pickan/klubban högt med båda händerna och slår rakt ner i marken.*

* **Total längd:** **1.433 s** (86 frames @ 60fps / 43 frames @ 30fps)
* **Telegraph & Startup (Wind-up):**
  * Tid: `0.000 s` till `0.600 s` (frames 0–35 @ 60fps / 0–17 @ 30fps).
  * **Tracking-fönster:** Roterar mjukt mot spelaren fram till `0.450 s` (frame 27 @ 60fps). Därefter låses riktningen helt (*Committed*) – spelaren kan rulla åt sidan och straffa.
* **Aktiv Hitbox (Strike Window):**
  * Tid: `0.600 s` till `0.767 s` (frames 36–45 @ 60fps / 18–22 @ 30fps).
  * Räckvidd: Spets från hand till mark ~1.60 m framför kroppen.
* **Recovery (Öppning för spelaren):**
  * Tid: `0.767 s` till `1.433 s` (frames 46–85 @ 60fps / 23–42 @ 30fps).
  * Kobolden står fast i marken med vapnet i golvet och är helt försvarslös.
* **Framåtrörelse (Lunge):** **1.10 m** (~1.20 yd).
* **Skada:** **25 fysisk skada**.
* **Poise Damage:** **30 poäng** (bryter spelare utan rustning).
* **Block Stamina Damage:** **35 stamina drain** mot 100% fysisk sköld med ~55 Stability.

---

## 2.2 Attack 2: Wild Flail / 2-Hit Horizontal Sweep (Dubbelsvep i panik)
*Två snabba horisontella svingar i följd (höger -> vänster, följt av vänster -> höger).*

* **Total längd (båda slagen):** **1.867 s** (112 frames @ 60fps / 56 frames @ 30fps)
* **Sving 1:**
  * **Startup:** `0.000 s` till `0.400 s` (frames 0–23 @ 60fps).
  * **Aktiv Hitbox 1:** `0.400 s` till `0.533 s` (frames 24–31 @ 60fps).
  * Skada 1: **16 skada**, **15 poise damage**.
* **Övergång / Sving 2:**
  * **Windup 2:** `0.533 s` till `0.800 s` (frames 32–47 @ 60fps).
  * **Aktiv Hitbox 2:** `0.800 s` till `0.933 s` (frames 48–55 @ 60fps).
  * Skada 2: **18 skada**, **20 poise damage**.
* **Recovery (efter sving 2):**
  * Tid: `0.933 s` till `1.867 s` (frames 56–111 @ 60fps).
* **Lunge:** **0.70 m** på slag 1, **0.90 m** på slag 2 (totalt 1.60 m / ~1.75 yd).

---

## 2.3 Attack 3: Desperation Jumping Slam (Löpande hoppattack)
*Triggas när spelaren backar eller befinner sig på 3.0–5.5 meters avstånd.*

* **Total längd:** **1.600 s** (96 frames @ 60fps / 48 frames @ 30fps)
* **Startup / Crouch:** `0.000 s` till `0.300 s` (frames 0–17 @ 60fps). Kobolden huka sig.
* **Luftbana (Jumping travel):** `0.300 s` till `0.700 s` (frames 18–41 @ 60fps).
  * Flyger framåt **3.20 m** (~3.50 yd).
  * Ingen kurskorrigering i luften (rak bana).
* **Aktiv Hitbox (Nedslag):** `0.700 s` till `0.833 s` (frames 42–49 @ 60fps).
* **Recovery (Kraschar i marken):** `0.833 s` till `1.600 s` (frames 50–95 @ 60fps).
* **Skada:** **35 skada** (hård träff!).
* **Poise Damage:** **45 poäng** (stapplar spelare även i medeltung brynja).
* **Block Stamina Damage:** **55 stamina drain**.
