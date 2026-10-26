# Warrior classes, talents and perfect defense

Approved direction: the user accepted preserving WoW class/spec identity while translating talents into Souls decisions, and requested implementation of the complete Warrior flow including perfect block and perfect parry (2026-10-18).

Class philosophy: weapons define reach/moves/tempo; class resources and abilities define its tools; talents reward intentional ways of fighting. Every specialization works solo. Progression adds bounded power and mastery, preserving dangerous combat.

## Player flow

Warriors earn one talent point per level from 10 through 60 (51 maximum), including existing characters. Three compact trees support hybrids. Each has six five-rank passives and a one-rank signature, with tree point gates 0,0,5,5,10,15,25. A signature requires 26 points, so only one fits in the 51-point budget. The fourth ability slot follows that signature automatically. Existing three trainer abilities retain their learning flow. Talents have names, rank descriptions, requirements and unavailable reasons. Menu > Talents provides keyboard, mouse and pad navigation, spending, and a deliberate reset outside combat. Per-character versioned saves load safely; missing saves start unspent, malformed saves are preserved, over-budget builds are rejected/refunded safely. Non-Warriors cannot spend or receive effects.

Arms: Tactical Mastery (slower out-of-combat rage decay), Improved Charge (cooldown), Heavy Hand (heavy poise), Impale (heavy damage), Second Wind (perfect-parry stamina), Bloodletting (heavy bleed); Mortal Strike signature.
Fury: Unbridled Wrath (weapon-hit rage), Flurry (landed-hit stamina refund, once per swing), Cruelty (chain damage), Blood Craze (melee-kill healing), Battle Trance (high-rage damage), Berserker (low-health damage); Bloodthirst signature.
Protection: Shield Specialization (block cost), Deflection (parry cost), Shield Discipline (perfect-block rage), Revenge (next landed melee after perfect defense), Bulwark (block chip), Concussion (perfect-defense poise pressure); Shield Wall signature.

Passives use small bounded increments. Signature attacks use existing equipped-weapon motions and collision; they spend resources only after body/weapon/target checks. A whiff cannot heal or grant rage. Respec removes the signature and all temporary benefits. Rage abilities do not generate their own rage. Damage over time uses normal hit/death/reward events and cannot trigger weapon-hit loops.

## Defense

Perfect block: the first eligible frontal block within 0.16 seconds of raising guard, with a 0.45-second rearm interval. Costs 35% of normal block stamina (minimum 1), deals no chip only if the cost can be paid; otherwise ordinary guard-break rules apply. Holding guard never refreshes the window. Back attacks, unblockable attacks, projectiles and grabs stay outside this mechanic. One raise can perfect-block one contact.

Parry keeps its existing authored active window, volume, front check and eligible attacks. Perfect parry additionally requires the enemy's attack to be within 0.10 seconds of its next actual hit window (including contact), and the player's parry to be in its first 0.12 active seconds. Earlier valid parries retain ordinary parry behavior. Perfect parry creates a short, target-bound next-melee counter opportunity; no automatic damage, teleport or invulnerability. Bosses keep their existing parry eligibility and animation rules. Failed parries retain stamina and recovery costs.

Perfect defenses emit an explicit event carrying attacker/quality/position, grant bounded Warrior rage and receive differentiated brief sound, sparks, hit-stop and HUD text. They reset on death and cannot farm resources from the same multi-hit contact. Talents improve the reward, never unlock the base ability to perfect-defend.

## Verification

Pure tests cover points/gates/hybrids/save validation, timing boundaries, held guard/rearm, low stamina, direction/unblockable exclusions, ordinary/perfect parry, target-bound counter consumption/expiry, class restrictions and signature refusal/payment. Run production format, clippy and workspace tests with -j 4. Build and run isolated unattended game checks and capture talent UI plus dense defensive combat frames. Update HANDOVER and a focused log. No commits unless requested; preserve pre-existing changes.
