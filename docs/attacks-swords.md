# Dark Souls Remastered — Sword Attack Patterns (1H & 2H)

Exakta värden, frame data, fönster för hitboxar, stamina-kostnader och lunge-sträckor för Straight Swords (Longsword / Broadsword) i Dark Souls Remastered (DSR).

Alla värden anges i:
- **Sekunder (s)**
- **60 FPS ticks** (DSR) och **30 FPS frames** (original FromSoft TAE-tidslinje)
- **Meter (m)** och **WoW Yards (yd)** (`1 m ≈ 1.0936 yd`)

---

## 1. Grundläggande Koncept i Souls-strid

1. **Startup (Wind-up):** Tiden från knapptryck till första framen där vapnets hitbox blir aktiv. Skadar inte fienden, kan avbrytas av fientlig stagger.
2. **Active (Träfffönster):** Frames där bladets svepvolym kan kollidera med en fiendes hurtbox.
3. **Recovery:** Tiden efter att hitboxen stängts till dess att karaktären återgår till neutral hållning (`Idle`).
4. **Cancel / Chain-fönster:** 
   * **Combo-kedja:** Vid vilken punkt under recovery som en ny attack i kedjan kan påbörjas omedelbart.
   * **Roll-cancel:** Vid vilken punkt under recovery som spelaren kan bryta svingen med en roll/backstep.
5. **Motion Value (MV):** Skademultiplikator som appliceras på vapnets Attack Rating (AR). Standard R1 är basvärde `100%` (1.00×).
6. **Poise Damage:** Mängden balans/stabilitet som slås bort från målet vid träff.

---

## 2. 1-Handed (1H) Moveset — Longsword

När svärdet hålls i en hand (sköld eller tom hand i vänster hand).

### 2.1 1H R1-1 — Light Attack 1 (Horisontellt svep höger -> vänster)
* **Total längd:** **0.800 s** (48 frames @ 60fps / 24 frames @ 30fps)
* **Startup:** `0.000 s` till `0.367 s` (frames 0–21 @ 60fps / 0–10 @ 30fps)
* **Aktiv hitbox:** `0.367 s` till `0.533 s` (frames 22–31 @ 60fps / 11–15 @ 30fps)
* **Recovery:** `0.533 s` till `0.800 s` (frames 32–47 @ 60fps / 16–23 @ 30fps)
* **Cancel & Kedjefönster:**
  * **Input Buffer:** Från `0.450 s` (frame 27 @ 60fps) kan nästa R1 buffras.
  * **Kedja till R1-2:** Aktiveras vid `0.667 s` (frame 40 @ 60fps / frame 20 @ 30fps).
  * **Avbryt med Roll (Roll-cancel):** Möjligt från `0.700 s` (frame 42 @ 60fps / frame 21 @ 30fps).
* **Lunge (Framåtrörelse under sving):** **1.20 m** (~1.31 yd). Karaktären kliver fram under startup och sving.
* **Stamina-kostnad:** **18 poäng**.
* **Motion Value (MV):** **100%** (1.00× AR).
* **Poise Damage:** **20 poäng**.

---

### 2.2 1H R1-2 — Light Attack 2 (Diagonalt svep vänster -> höger)
*Följdattacken när R1 trycks under eller strax efter R1-1.*

* **Total längd:** **0.767 s** (46 frames @ 60fps / 23 frames @ 30fps)
* **Startup:** `0.000 s` till `0.267 s` (frames 0–15 @ 60fps / 0–7 @ 30fps) — *snabbare än R1-1 tack vare momentum!*
* **Aktiv hitbox:** `0.267 s` till `0.433 s` (frames 16–25 @ 60fps / 8–12 @ 30fps)
* **Recovery:** `0.433 s` till `0.767 s` (frames 26–45 @ 60fps / 13–22 @ 30fps)
* **Cancel & Kedjefönster:**
  * **Kedja tillbaka till R1-1:** Från `0.633 s` (frame 38 @ 60fps / frame 19 @ 30fps).
  * **Roll-cancel:** Från `0.667 s` (frame 40 @ 60fps / frame 20 @ 30fps).
* **Lunge:** **0.80 m** (~0.87 yd).
* **Stamina-kostnad:** **18 poäng**.
* **Motion Value (MV):** **100%** (1.00× AR).
* **Poise Damage:** **20 poäng**.

---

### 2.3 1H R2 — Heavy Attack (Framåtstöt / Thrust)
*En kraftfull, rak stöt framåt med ökad räckvidd och stickskada (Thrust).*

* **Total längd:** **1.133 s** (68 frames @ 60fps / 34 frames @ 30fps)
* **Startup:** `0.000 s` till `0.600 s` (frames 0–35 @ 60fps / 0–17 @ 30fps) — lång wind-up där bladet dras tillbaka.
* **Aktiv hitbox:** `0.600 s` till `0.767 s` (frames 36–45 @ 60fps / 18–22 @ 30fps)
* **Recovery:** `0.767 s` till `1.133 s` (frames 46–67 @ 60fps / 23–33 @ 30fps)
* **Cancel & Kedjefönster:**
  * **Kedja till R2-2:** Från `0.967 s` (frame 58 @ 60fps / frame 29 @ 30fps).
  * **Roll-cancel:** Från `1.000 s` (frame 60 @ 60fps / frame 30 @ 30fps).
* **Lunge:** **2.40 m** (~2.62 yd) — karaktären tar ett rejält kliv framåt i utfall.
* **Stamina-kostnad:** **28 poäng**.
* **Motion Value (MV):** **125%** (1.25× AR) + *20% bonus vid Counter-hit (om fienden träffas under egen attackanimation)*.
* **Poise Damage:** **30 poäng**.

---

### 2.4 1H Rolling Attack (Roll -> R1)
*Ett snabbt stick eller lågt svep som utförs omedelbart när en roll avslutas.*

* **Total längd:** **0.700 s** (42 frames @ 60fps / 21 frames @ 30fps)
* **Startup:** `0.000 s` till `0.267 s` (frames 0–15 @ 60fps / 0–7 @ 30fps) räknat från rollens cancel-fönster.
* **Aktiv hitbox:** `0.267 s` till `0.400 s` (frames 16–23 @ 60fps / 8–11 @ 30fps)
* **Recovery:** `0.400 s` till `0.700 s` (frames 24–41 @ 60fps / 12–20 @ 30fps)
* **Cancel:** Kedjar till R1-1 från `0.567 s`.
* **Lunge:** **1.50 m** (~1.64 yd).
* **Stamina-kostnad:** **16 poäng**.
* **Motion Value (MV):** **95%** (0.95× AR).
* **Poise Damage:** **20 poäng**.

---

### 2.5 1H Running / Backstep Attack (Sprint eller Backstep -> R1)
*Ett framåtspringande hugg med bevarad rörelseenergi.*

* **Total längd:** **0.900 s** (54 frames @ 60fps / 27 frames @ 30fps)
* **Startup:** `0.000 s` till `0.433 s` (frames 0–25 @ 60fps / 0–12 @ 30fps)
* **Aktiv hitbox:** `0.433 s` till `0.567 s` (frames 26–33 @ 60fps / 13–16 @ 30fps)
* **Recovery:** `0.567 s` till `0.900 s` (frames 34–53 @ 60fps / 17–26 @ 30fps)
* **Lunge / Momentum:** **2.80 m** (~3.06 yd) glider framåt under utförandet.
* **Stamina-kostnad:** **22 poäng**.
* **Motion Value (MV):** **110%** (1.10× AR).
* **Poise Damage:** **25 poäng**.

---

### 2.6 Kick / Spark (Framåt + R1 samtidigt)
*Sköldbrytande spark för att öppna upp blockerande motståndare.*

* **Total längd:** **0.733 s** (44 frames @ 60fps / 22 frames @ 30fps)
* **Startup:** `0.000 s` till `0.333 s` (frames 0–19 @ 60fps / 0–9 @ 30fps)
* **Aktiv träff:** `0.333 s` till `0.433 s` (frames 20–25 @ 60fps / 10–12 @ 30fps)
* **Recovery:** `0.433 s` till `0.733 s` (frames 26–43 @ 60fps / 13–21 @ 30fps)
* **Skada:** Mycket låg hälsoskada (~5–10 HP), men orsakar **enorm Stamina Drain mot blockerande sköldar** (~70–100 stamina damage). Om motståndarens stamina tar slut slås skölden åt sidan (`Guard Break Stagger`).
* **Stamina-kostnad:** **14 poäng**.

---

## 3. 2-Handed (2H) Moveset — Longsword

När vapnet hålls med båda händerna (`Two-Handed Stance`):
* **Styrke-skalning (Strength):** Karaktärens effektiva Strength multipliceras med **1.5×**.
* **Ökad Poise Damage:** Samtliga attacker ger **+50% Poise Damage** (20 -> 30 på R1).
* **Ökad Skada (Motion Value):** Generellt **+15–25%** högre skada än 1H.
* **Ökad Stamina-kostnad:** Kräver ~**25–33%** mer stamina per sving.

---

### 3.1 2H R1-1 — Two-Handed Light 1 (Diagonalt nersvep)
* **Total längd:** **0.833 s** (50 frames @ 60fps / 25 frames @ 30fps)
* **Startup:** `0.000 s` till `0.400 s` (frames 0–23 @ 60fps / 0–11 @ 30fps)
* **Aktiv hitbox:** `0.400 s` till `0.567 s` (frames 24–33 @ 60fps / 12–16 @ 30fps)
* **Recovery:** `0.567 s` till `0.833 s` (frames 34–49 @ 60fps / 17–24 @ 30fps)
* **Cancel & Kedjefönster:**
  * **Kedja till 2H R1-2:** Från `0.700 s` (frame 42 @ 60fps / frame 21 @ 30fps).
  * **Roll-cancel:** Från `0.733 s` (frame 44 @ 60fps / frame 22 @ 30fps).
* **Lunge:** **1.35 m** (~1.48 yd).
* **Stamina-kostnad:** **24 poäng** (jämfört med 18 i 1H).
* **Motion Value (MV):** **120%** (1.20× AR).
* **Poise Damage:** **30 poäng** (slår igenom lätt rustning direkt!).

---

### 3.2 2H R1-2 — Two-Handed Light 2 (Uppåtgående svep)
* **Total längd:** **0.800 s** (48 frames @ 60fps / 24 frames @ 30fps)
* **Startup:** `0.000 s` till `0.300 s` (frames 0–17 @ 60fps / 0–8 @ 30fps)
* **Aktiv hitbox:** `0.300 s` till `0.467 s` (frames 18–27 @ 60fps / 9–13 @ 30fps)
* **Recovery:** `0.467 s` till `0.800 s` (frames 28–47 @ 60fps / 14–23 @ 30fps)
* **Cancel & Kedjefönster:**
  * **Kedja till 2H R1-1:** Från `0.667 s` (frame 40 @ 60fps).
* **Stamina-kostnad:** **24 poäng**.
* **Motion Value (MV):** **120%** (1.20× AR).
* **Poise Damage:** **30 poäng**.

---

### 3.3 2H R2 — Two-Handed Heavy (Tvåhands-stöt / Nedslående klyv)
*Ett tungt nedåtgående tvåhands-hugg som krossar fiendens försvar.*

* **Total längd:** **1.200 s** (72 frames @ 60fps / 36 frames @ 30fps)
* **Startup:** `0.000 s` till `0.667 s` (frames 0–39 @ 60fps / 0–19 @ 30fps)
* **Aktiv hitbox:** `0.667 s` till `0.833 s` (frames 40–49 @ 60fps / 20–24 @ 30fps)
* **Recovery:** `0.833 s` till `1.200 s` (frames 50–71 @ 60fps / 25–35 @ 30fps)
* **Lunge:** **2.20 m** (~2.41 yd).
* **Stamina-kostnad:** **36 poäng**.
* **Motion Value (MV):** **145%** (1.45× AR).
* **Poise Damage:** **45 poäng** (knäcker även medeltung rustning i ett slag).

---

### 3.4 2H Rolling Attack (Roll -> R1 i tvåhandsfattning)
* **Total längd:** **0.733 s** (44 frames @ 60fps / 22 frames @ 30fps)
* **Startup:** `0.000 s` till `0.267 s` (frames 0–15 @ 60fps)
* **Aktiv hitbox:** `0.267 s` till `0.400 s` (frames 16–23 @ 60fps)
* **Stamina-kostnad:** **21 poäng**.
* **Poise Damage:** **30 poäng**.

---

### 3.5 2H Running Attack (Sprint -> R1 i tvåhandsfattning)
* **Total längd:** **0.933 s** (56 frames @ 60fps / 28 frames @ 30fps)
* **Startup:** `0.000 s` till `0.400 s` (frames 0–23 @ 60fps)
* **Aktiv hitbox:** `0.400 s` till `0.567 s` (frames 24–33 @ 60fps)
* **Lunge / Glid:** **3.00 m** (~3.28 yd).
* **Stamina-kostnad:** **28 poäng**.
* **Motion Value (MV):** **130%** (1.30× AR).
* **Poise Damage:** **35 poäng**.
