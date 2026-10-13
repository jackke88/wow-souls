# Dark Souls & WoW 1.12.1 — Animations & Clip Catalog

Fullständig katalog över animationer, klipp-ID:n, filnamn och skelettmappning för både **Dark Souls Remastered (DSR)** och **World of Warcraft 1.12.1 (M2)**.

Dokumentet är uppdelat i:
1. **Spelaren (Player / Human):** DSR `c0000` Havok HKX clips vs WoW `HumanMale.m2` (`AnimationData.dbc`)
2. **Kobold (Undead Hollow):** DSR `c2060`/`c2100` clips vs WoW `Kobold.m2`
3. **Varg (Wolf / Undead Dog):** DSR `c2260` clips vs WoW `Wolf.m2`
4. **Defias Thief (Undead Assassin):** DSR `c2200`/`c0000` clips vs WoW M2
5. **Retargeting & Ben-mappning:** Översättning mellan Dark Souls 60-bens Havok-rigg och WoW 119-bens M2-rigg

---

## 1. Spelarkaraktären (Player)

### 1.1 Dark Souls Remastered (`chr/c0000_*.anibnd.dcx`)
I DSR ligger spelarens animationer som komprimerade Havok 2015 tagfiles (`hkaSplineCompressedAnimation`) med root motion i `hkaAnimatedReferenceFrame`:

#### A. Locomotion & Rörelse (`c0000_a00_lo.anibnd.dcx` / `_md`)
| DSR HKX Fil | Beskrivning | Längd / Frames (60fps) | WoW M2 Motsvarighet (`AnimationData.dbc`) |
|---|---|---|---|
| `a00_0000.hkx` | Idle / Stå stilla neutralt | Loop | `0` (`Stand`) |
| `a00_0001.hkx` | Walk Forward (gång framåt) | Loop (2.18 m/s) | `4` (`Walk`) |
| `a00_0002.hkx` | Walk Forward variation | Loop | `4` (`Walk`) |
| `a00_0003.hkx` | Run Forward (standardjogg) | Loop (4.36 m/s) | `5` (`Run`) |
| `a00_0005.hkx` | Sprint / Dash (snabbspurt) | Loop (6.54 m/s) | `5` (`Run` vid 1.5× playback rate) |
| `a00_0010.hkx` | Walk Backward (backpedal) | Loop (3.05 m/s) | `13` (`WalkBackwards`) |
| `a00_0011.hkx` | Walk Left (strafe vänster) | Loop (3.85 m/s) | `11` (`ShuffleLeft`) |
| `a00_0012.hkx` | Walk Right (strafe höger) | Loop (3.85 m/s) | `12` (`ShuffleRight`) |
| `a00_0060.hkx` | 180° Skid (U-sväng inbromsning) | 21 frames (0.35 s) | — (saknas i WoW, byggs via motor) |

#### B. Dodges & Rolls (`c0000_a00_lo` / `_md` / `_hi`)
| DSR HKX Fil | Kategori | Längd | i-frames | Distans | Notering |
|---|---|---|---|---|---|
| `a00_0710.hkx` | **Fast Roll Forward** | 48f (0.80 s) | Frames 0–21 | 4.20 m | < 25% Equip Load |
| `a00_0711.hkx` | Fast Roll Backward | 48f (0.80 s) | Frames 0–21 | 4.00 m | Rullning bakåt |
| `a00_0712.hkx` | Fast Roll Left | 48f (0.80 s) | Frames 0–21 | 4.10 m | Sidledsrullning |
| `a00_0713.hkx` | Fast Roll Right | 48f (0.80 s) | Frames 0–21 | 4.10 m | Sidledsrullning |
| `a00_0700.hkx` | **Medium Roll Forward** | 66f (1.10 s) | Frames 0–21 | 3.80 m | 25–50% Equip Load |
| `a00_0701.hkx` | Medium Roll Backward | 66f (1.10 s) | Frames 0–21 | 3.60 m | — |
| `a00_0702.hkx` | Medium Roll Left | 66f (1.10 s) | Frames 0–21 | 3.70 m | — |
| `a00_0703.hkx` | Medium Roll Right | 66f (1.10 s) | Frames 0–21 | 3.70 m | — |
| `a00_0720.hkx` | **Fat Roll Forward** | 92f (1.53 s) | Frames 0–17 | 3.00 m | > 50% Equip Load (duns i marken) |
| `a00_0705.hkx` | **Backstep** | 36f (0.60 s) | 0 frames | 1.65 m | Hopp bakåt från neutral spak |

#### C. Svärdsattacker (Straight Sword — `c0000_a2x.anibnd.dcx`)
Alla svärdsklipp för standard Straight Sword börjar med prefix `a20_`:
| DSR HKX Fil | Beskrivning | Frames (60fps) | Aktiv Hitbox | Lunge |
|---|---|---|---|---|
| `a20_3000.hkx` | **1H R1-1** (Höger -> vänster svep) | 48f (0.80 s) | Frames 22–31 | 1.20 m |
| `a20_3001.hkx` | **1H R1-2** (Vänster -> höger combo) | 46f (0.77 s) | Frames 16–25 | 0.80 m |
| `a20_3030.hkx` | **1H R2** (Kraftig framåtstöt / Thrust) | 68f (1.13 s) | Frames 36–45 | 2.40 m |
| `a20_3100.hkx` | **1H Rolling Attack** (Uppstick efter roll) | 42f (0.70 s) | Frames 16–23 | 1.50 m |
| `a20_3300.hkx` | **1H Running Attack** (Löpande snitt) | 54f (0.90 s) | Frames 26–33 | 2.80 m |
| `a20_3400.hkx` | **Kick / Guard Break** (Spark) | 44f (0.73 s) | Frames 20–25 | 0.50 m |
| `a20_4000.hkx` | **2H R1-1** (Tvåhands nersvep) | 50f (0.83 s) | Frames 24–33 | 1.35 m |
| `a20_4001.hkx` | **2H R1-2** (Tvåhands uppsvep) | 48f (0.80 s) | Frames 18–27 | 0.90 m |
| `a20_4030.hkx` | **2H R2** (Tvåhands kraftklyv) | 72f (1.20 s) | Frames 40–49 | 2.20 m |
| `a20_4100.hkx` | **2H Rolling Attack** | 44f (0.73 s) | Frames 16–23 | 1.60 m |
| `a20_4300.hkx` | **2H Running Attack** | 56f (0.93 s) | Frames 24–33 | 3.00 m |

#### D. Träffreaktioner & Skada (`c0000_a00_*.anibnd.dcx`)
| DSR HKX Fil | Händelse | Längd (60fps) | WoW Motsvarighet |
|---|---|---|---|
| `a00_8000.hkx` | Light Hit Stagger (Lätt vackling framåt) | 30f (0.50 s) | `9` (`CombatWound`) |
| `a00_8010.hkx` | Heavy Hit Stagger (Kroppen kastas bakåt) | 50f (0.83 s) | `10` (`CombatCritical`) |
| `a00_8050.hkx` | Guard Break (Skölden slås undan) | 72f (1.20 s) | `14` (`Stun`) |
| `a00_8060.hkx` | Parried (Parerad av fiende, står försvarslös) | 84f (1.40 s) | `14` (`Stun`) |
| `a00_9000.hkx` | Death (Dör och faller bakåt) | 90f (1.50 s) | `1` (`Death`) |

---

## 2. Kobold (Undead Hollow Miner)

### 2.1 Dark Souls Arkiv: `chr/c2060.anibnd.dcx`
| DSR HKX Fil | Beskrivning | Frames (60fps) | Aktiv Hitbox | WoW Kobold M2 ID |
|---|---|---|---|---|
| `a00_0000.hkx` | Kutryggig Idle / Stance | Loop | — | `0` (`Stand`) |
| `a00_0200.hkx` | Vaggande gång (Walk) | Loop (2.2 m/s) | — | `4` (`Walk`) |
| `a00_0202.hkx` | Jagande löpsteg (Chase) | Loop (3.8 m/s) | — | `5` (`Run`) |
| `a00_3000.hkx` | **Overhead Slam** (Telegraferat nerslag) | 86f (1.43 s) | Frames 36–45 | `17` (`Attack1H` @ 0.7× speed) |
| `a00_3001.hkx` | **Wild Flail 1** (Horisontellt svep 1) | 56f (0.93 s) | Frames 24–31 | `16` (`AttackUnarmed` / Attack) |
| `a00_3002.hkx` | **Wild Flail 2** (Retursvep 2) | 56f (0.93 s) | Frames 16–23 | `17` (`Attack1H`) |
| `a00_3300.hkx` | **Desperation Jump Attack** (Hoppattack) | 96f (1.60 s) | Frames 42–49 | Special / Jump attack |
| `a00_2000.hkx` | Flinch / Stagger vid träff | 30f (0.50 s) | — | `9` (`CombatWound`) |
| `a00_9000.hkx` | Death Fall (Kollapsar till marken) | 80f (1.33 s) | — | `1` (`Death`) |

---

## 3. Varg / Wolf (Undead Attack Dog)

### 3.1 Dark Souls Arkiv: `chr/c2260.anibnd.dcx`
| DSR HKX Fil | Beskrivning | Frames (60fps) | Aktiv Hitbox | WoW Wolf M2 ID |
|---|---|---|---|---|
| `a00_0000.hkx` | Låg rovdjurshållning (Idle) | Loop | — | `0` (`Stand`) |
| `a00_0200.hkx` | Smidig lunk / trav (Trot) | Loop (3.2 m/s) | — | `4` (`Walk`) |
| `a00_0202.hkx` | **Sprint Chase** (Explosiv spurt) | Loop (6.2 m/s) | — | `5` (`Run` @ 1.3× speed) |
| `a00_3000.hkx` | **Quick Snap Bite** (Nypande bett) | 42f (0.70 s) | Frames 14–21 | `16` (`Attack` / Bett) |
| `a00_3001.hkx` | **Double Bite Combo** | 70f (1.17 s) | Frames 12–19 & 30–37 | `17` (`Attack` variation) |
| `a00_3005.hkx` | **Leaping Pounce** (Flygande språngbett) | 76f (1.27 s) | Frames 28–37 | `17` (Special språng) |
| `a00_0701.hkx` | **Evasive Hop Backward** | 30f (0.50 s) | — | Evasive hop |
| `a00_0702.hkx` | **Evasive Hop Left** | 30f (0.50 s) | — | Sidledsskutt |
| `a00_0703.hkx` | **Evasive Hop Right** | 30f (0.50 s) | — | Sidledsskutt |
| `a00_2000.hkx` | Flinch (Rycker bakåt) | 27f (0.45 s) | — | `9` (`CombatWound`) |
| `a00_2080.hkx` | Knockdown Rollback (Kullvält av tungt slag) | 72f (1.20 s) | — | `121` (`Knockdown`) |
| `a00_9000.hkx` | Death (Dör på sidan) | 60f (1.00 s) | — | `1` (`Death`) |

---

## 4. Defias Thief (Undead Assassin)

### 4.1 Dark Souls Arkiv: `c0000` / `c2200`
Eftersom Defias Thief är en humanoid lönnmördare i läder använder han Dark Souls spelar-rigg (`c0000`) med Rogue/Thief-specifika animationsklasser:

| DSR Klipp | Beskrivning | Frames (60fps) | Egenskap / Hitbox |
|---|---|---|---|
| `a00_0000.hkx` | Låg stridsställning med dolk & sköld | Loop | Redo för motanfall |
| `a00_0011` / `0012` | Aggressiv cirklande strafe | Loop (4.1 m/s) | Söker spelarens flank/rygg |
| `a00_3000` + `3001` + `3002` | **3-Hit Dagger Flurry** | 90f (1.50 s) | 3 snabba snitt + Bleed buildup |
| `a00_3005.hkx` | **Leaping Ambush** (Dykande stöt) | 78f (1.30 s) | Hoppar 3.5 m framåt |
| `a00_3010.hkx` | **Throwing Knife** (Kastkniv) | 54f (0.90 s) | Projektil spawnas frame 22 |
| `a00_0500.hkx` | **Parry Stance** (Höjer Target Shield) | 36f (0.60 s) | Parerar spelaren frames 12–24 |
| `a00_3030.hkx` | **Riposte** (Kritisk stöt efter parering) | 100f (1.67 s) | 80 lethal skada på fryst spelare |
| `a00_0705.hkx` | **Quickstep / Backflip** | 28f (0.47 s) | Snabb volt bakåt 2.2 m |

---

## 5. Skelett- och Retargeting-mappning

För att spela Dark Souls Havok-animationer direkt på WoW 1.12.1 M2-modeller (sub-project B i wow-souls) mappar benen enligt följande:

### Skelett-jämförelse
* **Dark Souls (`Skeleton.hkx`):** ~60 namngivna ben i en ren hierarkisk struktur med standardiserade T-poses.
* **WoW Human Male (`HumanMale.m2`):** 119 onamngivna ben (indexerade 0–118), där vissa bär `KeyBoneID` (Spine, Head, Arms etc.).

### Primär Ben-mappning (DSR ➔ WoW M2):
| DS Rigg-ben (`c0000`) | WoW Human Male M2 Ben | Funktion i motorn |
|---|---|---|
| `Master` / `Root` | Root transform (`ModelFrame`) | Tar emot Havok root-motion förflyttning i X/Z |
| `Pelvis` | Ben 0 (`Pelvis / Waist`) | Höftens rotation och vertikala gupp |
| `Spine` | Ben 1 (`SpineLow`) | Nedre ryggradens böjning |
| `Spine1` | Ben 2 (`SpineHigh`) | Bröstkorgens lutning |
| `Neck` / `Head` | Ben 3 & 4 (`Neck` / `Head`) | Huvudets vinkel mot målet |
| `L_Clavicle` / `R_Clavicle` | Ben 5 & 6 (`ClavicleL` / `ClavicleR`) | Axlarnas bredd och hållning |
| `L_UpperArm` / `R_UpperArm` | Ben 7 & 8 (`ArmL` / `ArmR`) | Överarmens svingbana |
| `L_Forearm` / `R_Forearm` | Ben 9 & 10 (`ForearmL` / `ForearmR`) | Underarm och armbågsled |
| `L_Hand` / `R_Hand` | Ben 11 & 12 (`HandL` / `HandR`) | Vapenfäste (Attachment 1) & sköldfäste |
| `L_Thigh` / `R_Thigh` | Ben 13 & 14 (`ThighL` / `ThighR`) | Lår och benpendling |
| `L_Calf` / `R_Calf` | Ben 15 & 16 (`CalfL` / `CalfR`) | Knäled |
| `L_Foot` / `R_Foot` | Ben 17 & 18 (`FootL` / `FootR`) | Fötter mot marken |

När en animation retargetas extraheras delta-rotationen $q_{\Delta} = q_{\text{ref}}^{-1} \cdot q_{\text{anim}}$ från Dark Souls-benet och multipliceras på WoW-modellens lokala bindpose-kvaternion:
$$q_{\text{wow}} = q_{\text{wow\_bind}} \cdot q_{\Delta}$$
Detta bevarar WoW-modellens proportioner utan att nätet vrids sönder (*mesh tearing*), samtidigt som Dark Souls exakta rörelsebana och timing bevaras till 100%.
