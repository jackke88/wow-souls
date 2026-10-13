# Dark Souls Remastered — Stamina, Poise & Combat Core Mechanics

Exakta regler, formler och timings för stamina-hantering, poise (balans), träffreaktioner (stagger), input buffering och mållåsning i Dark Souls Remastered (DSR), för implementering i `wow-souls`.

---

## 1. Stamina-systemet

Stamina styr takten i hela stridssystemet och är den primära resursen för anfall, undvikande och försvar.

### 1.1 Basvärden & Regeneration
* **Max Stamina:** Basvärde **100 poäng** (skalar med *Endurance* upp till **160** vid 40 Endurance).
* **Basregeneration:** **45.0 poäng / sekund** (`0.75 poäng per frame @ 60fps`).
* **Regenerations-fördröjning (`STAMINA_DELAY`):** **0.60 sekunder** (36 frames @ 60fps / 18 frames @ 30fps).
  * Fördröjningen nollställs och startar om varje gång spelaren utför en handling som förbrukar stamina (attack, roll, backstep, blockering, sprint).
* **Sköldblockerings-straff:** Medan en sköld hålls upplyft (`Guard Up`) reduceras regenerationshastigheten till **15.0 poäng / sekund** (en reduktion med 66%).

### 1.2 "Negative Stamina"-regeln (Souls-regeln)
* **Starta handling:** En handling (attack, roll) kan påbörjas så länge spelaren har **minst > 0.0 stamina**.
* **Övertrassering:** Om spelaren har 3 stamina kvar och utför en 1H R1 (kostnad 18) dras stamina ned till **-15.0**.
* **Återhämtningsstraff:** Inga nya handlingar eller sprint kan utföras förrän stamina har regenererats hela vägen upp över `0.0`. Fördröjningen på 0.60 s gäller som vanligt innan regenereringen från det negativa värdet påbörjas.

### 1.3 Sköldblockering & Stamina-skada
När ett slag träffar en upplyft sköld dras stamina baserat på anfallets råa skada och sköldens stabilitet (*Stability*):

$$\text{StaminaDamage} = \text{IncomingDamage} \times \left(1 - \frac{\text{Stability}}{100}\right)$$

* Exempel: En fiende slår för 100 skada mot en sköld med 60 Stability:
  $$\text{StaminaDamage} = 100 \times (1 - 0.60) = 40\text{ stamina}$$
* **Guard Break Stagger:** Om anfallets stamina-skada överstiger spelarens kvarvarande stamina bryts garden upp:
  * Karaktären slås bakåt med vidöppna armar (`Guard Break animation`).
  * Varaktighet: **1.20 s** (72 frames @ 60fps).
  * Karaktären tar **+40% instabilitetsskada** under hela denna tid.

---

## 2. Input Buffering (Inmatningskö)

För att striden ska kännas flytande men distinkt använder Dark Souls ett strikt en-knapps inmatningsminne (*Queue / Buffer*):

* **Buffer-varaktighet (`BUFFER`):** **0.35 sekunder** (~21 frames @ 60fps / ~10–11 frames @ 30fps).
* **Funktion:**
  1. Om spelaren trycker på Attack eller Evade under en pågående animations oavbrytbara fas (startup, aktiv hitbox eller tidig recovery) sparas inmatningen i bufferten.
  2. Så fort den pågående handlingen når sitt **Cancel-fönster** (t.ex. frame 40 på en R1) eller återgår till `Idle`, exekveras den sparade handlingen omedelbart på exakt den första möjliga framen.
  3. **Enkel slot:** Endast det senaste giltiga knapptrycket sparas (ett senare tryck skriver över ett tidigare).
  4. Om cancel-fönstret inte nås inom 0.35 s förfaller bufferten utan åtgärd.

---

## 3. Poise & Stagger (Balans & Vackling)

Poise avgör om en karaktär kan ta emot ett slag utan att få sin attack eller rörelse avbruten (*Hyper Armor / Poise through*).

### 3.1 Typiska Poise-trösklar
* **Naken / Tyg (Robes):** `0 Poise` (varje enskild träff avbryter).
* **Lätt läder / Tjuvrustning:** `10–25 Poise` (tål en dagger/pil, men bryts av svärd).
* **Kedjebrynja / Riddare (Knight):** `30–50 Poise` (tål 1-2 träffar från 1H Straight Sword).
* **Tung plåt (Havel / Stone):** `70–120+ Poise` (tål Greatswords och tunga slag).

### 3.2 Poise Damage per Vapentyp
| Vapen / Attack | 1H Poise Damage | 2H Poise Damage |
|---|---|---|
| **Dagger / Pil** | 10 | 15 |
| **Straight Sword / Curved Sword** | **20** | **30** |
| **Greatsword (Bastard / Claymore)** | **35** | **52.5** |
| **Ultra Greatsword / Greathammer** | **50–75** | **75–100** |

### 3.3 Stagger-varaktighet vid Poise Break
När inkommande Poise Damage överstiger karaktärens kvarvarande Poise bryts balansen:

* **Light Stagger (Lätt vackling / CombatWound):**
  * Varaktighet: **0.50 s** (30 frames @ 60fps).
  * Karaktären rycker till och avbryter pågående handling.
* **Heavy Stagger (Tung vackling):**
  * Varaktighet: **0.80 s – 1.00 s** (48–60 frames @ 60fps).
* **Knockdown (Kastad till marken av Ultra-vapen / boss):**
  * Varaktighet: **1.80 s** (karaktären slås till marken och måste resa sig upp). Osårbar under uppresningsfasen.

### 3.4 Poise Reset Timer (Återställning)
* Poise regenereras **inte** gradvis över tid.
* **Full Reset:** Om karaktären undviker att ta någon ytterligare poise-skada under **3.50 sekunder**, återställs Poise ögonblickligen till **100% full kapacitet**.

---

## 4. Hit-Stop (Träff-frysning / Impact Crunch)

För att ge fysisk tyngd åt svärdshugg använder Dark Souls en subtil mikropaus när bladet skär genom målet:

* **Varaktighet:** **0.050 s – 0.070 s** (3 till 4 frames @ 60fps).
* **Mekanik:** Både anfallarens svinghastighet och målets reaktionshastighet saktas ner kraftigt (till ~5–10% av normal animationshastighet) under dessa få millisekunder. Detta skapar den klassiska "crunch"-känslan utan att desynka spelets övergripande flöde.

---

## 5. Lock-On Mekanik (Mållåsning)

* **Maximalt låsavstånd:** **20.0 yards** (~18.3 meter).
* **Brytavstånd:** **25.0 yards** (~22.8 meter). Om avståndet överstiger detta släpps målet automatiskt.
* **Line of Sight (Siktlinje):** Om en solid vägg, pelare eller terräng bryter sikten mot målet behålls låsningen i **1.50 sekunder** i reserv (*grace period*) innan den bryts, vilket förhindrar att kameran rycker bort vid korta skymningar bakom träd/pelare.
* **Kamera-tracking:** Kamerans vinkel (*pitch & yaw*) interpoleras mjukt för att hålla målet centrerat, med spelarens blick fixerad på fiendens brösthöjd.
