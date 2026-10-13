# Dark Souls Remastered — Dodge & Evade Mechanics

Exakta värden, frame data och kurvor för rolls och backstep från Dark Souls Remastered (DSR), anpassade för implementering i `wow-souls`.

Alla tidsangivelser anges i:
- **Sekunder (s)** för kontinuerlig motortid (`dt`)
- **60 FPS ticks** (DSR standard)
- **30 FPS frames** (original FromSoftware TAE-logik)
- **Enheter:** Meter (m) och WoW Yards (yd), där `1 m ≈ 1.0936 yd` (eller `1 yd ≈ 0.9144 m`).

---

## 1. Översikt över Roll-kategorier (Equip Load)

Dark Souls baserar roll-typ strikt på förhållandet mellan buren utrustning och maximal bärförmåga (*Equip Load*):

| Kategori | Viktgräns | Total tid (s) | 60 FPS | 30 FPS | i-frames (s) | i-frames (60f) | i-frames (30f) | Distans | Stamina |
|---|---|---|---|---|---|---|---|---|---|
| **Fast Roll** | < 25.0% | **0.80 s** | 48f | 24f | **0.367 s** | Frames 0–21 (22f) | Frames 0–10 (11f) | ~4.2 m (4.6 yd) | 20 |
| **Mid Roll** | 25.0% – 50.0% | **1.10 s** | 66f | 33f | **0.367 s** | Frames 0–21 (22f) | Frames 0–10 (11f) | ~3.8 m (4.15 yd) | 20 |
| **Fat Roll** | 50.0% – 100.0% | **1.53 s** | 92f | 46f | **0.300 s** | Frames 0–17 (18f) | Frames 0–8 (9f) | ~3.0 m (3.3 yd) | 20 |
| **Overencumbered** | > 100.0% | — | — | — | Ingen roll | Kan endast vackla framåt | — | 0 m | — |

> [!NOTE]
> I DSR är i-frames **aktiva från allra första framen** (Frame 0). Om spelaren träffas samma frame som roll-kommandot registreras ignoreras skadan.
> Notera att Fast roll och Mid roll har *exakt samma antal i-frames* (11 frames i 30fps / 22 ticks i 60fps), men Mid roll har **18 frames längre recovery** (sårbarhet efter volten).

---

## 2. Detaljerad Frame Data

### 2.1 Fast Roll (< 25% Equip Load)
*Den mest responsiva dodgen. Snabb återhämtning till attack eller ny roll.*

* **Total längd:** 0.800 s (48 frames @ 60fps / 24 frames @ 30fps)
* **Invulnerability (i-frames):** 
  * Tid: `0.000 s` till `0.367 s`
  * 60 FPS: Frame `0` till `21` (22 ticks)
  * 30 FPS: Frame `0` till `10` (11 frames)
* **Aktiv förflyttning (Travel):** `0.000 s` till `0.567 s` (frames 0–34 @ 60fps). Resten är återhämtning i landning/stående.
* **Sårbar fas (Recovery):** `0.367 s` till `0.800 s` (frames 22–47 @ 60fps / 11–23 @ 30fps).
* **Cancel / Buffer-fönster:**
  * **Input Buffer accepterar nästa press:** Från `0.450 s` (frame 27 @ 60fps).
  * **Kedja till ny Roll:** Från `0.700 s` (frame 42 @ 60fps / frame 21 @ 30fps).
  * **Kedja till Rolling Attack:** Från `0.600 s` (frame 36 @ 60fps / frame 18 @ 30fps).
* **Förflyttningsdistans:** ~4.20 m (~4.59 yd).
* **Stamina-kostnad:** 20 poäng (dras omedelbart vid frame 0).

---

### 2.2 Mid Roll (25% – 50% Equip Load)
*Standardrullen för medeltunga rustningar (riddare/krigare). Samma osårbarhet som fast roll men markant längre recovery.*

* **Total längd:** 1.100 s (66 frames @ 60fps / 33 frames @ 30fps)
* **Invulnerability (i-frames):** 
  * Tid: `0.000 s` till `0.367 s`
  * 60 FPS: Frame `0` till `21` (22 ticks)
  * 30 FPS: Frame `0` till `10` (11 frames)
* **Aktiv förflyttning (Travel):** `0.000 s` till `0.650 s` (frames 0–39 @ 60fps).
* **Sårbar fas (Recovery):** `0.367 s` till `1.100 s` (frames 22–65 @ 60fps).
* **Cancel / Buffer-fönster:**
  * **Input Buffer:** Från `0.750 s` (frame 45 @ 60fps).
  * **Kedja till ny Roll:** Från `1.000 s` (frame 60 @ 60fps / frame 30 @ 30fps).
  * **Kedja till Rolling Attack:** Från `0.867 s` (frame 52 @ 60fps / frame 26 @ 30fps).
* **Förflyttningsdistans:** ~3.80 m (~4.15 yd).
* **Stamina-kostnad:** 20 poäng.

---

### 2.3 Fat Roll (50% – 100% Equip Load)
*Tung pansar. Karaktären dunsar hårt i marken och reser sig långsamt.*

* **Total längd:** 1.533 s (92 frames @ 60fps / 46 frames @ 30fps)
* **Invulnerability (i-frames):** 
  * Tid: `0.000 s` till `0.300 s`
  * 60 FPS: Frame `0` till `17` (18 ticks)
  * 30 FPS: Frame `0` till `8` (9 frames)
* **Aktiv förflyttning (Travel):** `0.000 s` till `0.700 s`. Karaktären fastnar sedan i landningsanimationen på knä/mage.
* **Sårbar fas (Recovery):** `0.300 s` till `1.533 s` (lång och extremt sårbar).
* **Cancel / Buffer-fönster:**
  * **Kedja till handling:** Från `1.400 s` (frame 84 @ 60fps).
* **Förflyttningsdistans:** ~3.00 m (~3.28 yd).
* **Stamina-kostnad:** 20 poäng.

---

### 2.4 Backstep (Stick neutral + Roll-knapp)
*Ett snabbt hopp bakåt. Används för att snabbt skapa avstånd eller initiera en Backstep Running Attack.*

* **Total längd:** 0.600 s (36 frames @ 60fps / 18 frames @ 30fps)
* **Invulnerability (i-frames):** **0 frames**
  *(I grundläggande Dark Souls 1 / DSR har backstep INGA i-frames utan special-ringar såsom Dark Wood Grain Ring).*
* **Aktiv förflyttning (Travel):** `0.000 s` till `0.350 s` (frames 0–21 @ 60fps).
* **Förflyttningsdistans:** ~1.65 m bakåt (~1.80 yd).
* **Cancel / Buffer-fönster:**
  * **Kedja till Backstep Attack (Light attack):** Från `0.300 s` (frame 18 @ 60fps / frame 9 @ 30fps). Utför omedelbart vapnets Running Attack framåt!
  * **Kedja till rörelse/ny roll:** Från `0.500 s` (frame 30 @ 60fps / frame 15 @ 30fps).
* **Stamina-kostnad:** 15 poäng.

---

## 3. Förflyttningskurva (Root Motion Profile)

I Dark Souls rör sig kroppen **inte linjärt** under en rullning. Havok root motion har en distinkt "fast-first"-kurva:
1. **Frames 0–12 (60fps):** Explosiv acceleration framåt/i vald riktning. Cirka 55% av totala sträckan avverkas under de första 0.20 sekunderna.
2. **Frames 13–30 (60fps):** Retardation medan kroppen roterar över axeln och fötterna söker marken.
3. **Frames 31+:** Kroppen stannar helt och står stilla under landningsrecoveryn.

### Formel för motor-hastighet $v(u)$ där $u \in [0, 1]$ är progress under `travel`:
En utmärkt approximation av DSR:s kurva för Bevy/Avian kinematic motor:
$$v(u) = v_{\text{avg}} \cdot 1.5 \cdot (1 - u^2)$$
där $v_{\text{avg}} = \frac{\text{distans}}{\text{travel\_duration}}$.

Exempel för Fast Roll ($4.2\text{ m}$ över $0.567\text{ s} \implies v_{\text{avg}} \approx 7.4\text{ m/s}$):
* Start: $v(0) \approx 11.1\text{ m/s}$ ($12.1\text{ yd/s}$)
* Mitt: $v(0.5) \approx 8.3\text{ m/s}$ ($9.1\text{ yd/s}$)
* Slut av travel: $v(1.0) \to 0\text{ m/s}$

---

## 4. Riktningskontroll (Omni vs 4-Way Roll)

* **Unlocked (Fri kamera):** Rullningen sker i full 360°-vinkel i förhållande till spelarens styrspak / kamerariktning. Karaktärens modell vrids omedelbart mot rullningsriktningen.
* **Locked On (Mållåsning):** 
  * I original Dark Souls 1 är rullningen låst till **4 kardinalriktningar** relativt till målet:
    1. Rent framåt (mot fienden)
    2. Rent bakåt (bort från fienden)
    3. Rent vänster (cirkel/strafe runt målet)
    4. Rent höger (cirkel/strafe runt målet)
  * Kroppen roterar för att utföra rullen och återvänder automatiskt till att möta målet vid slutet av animationen.
