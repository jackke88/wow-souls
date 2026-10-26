# Syn- och hörselaggro

User request: enemies detect farther in front; approaching from behind permits getting closer.

Use a 140 degree sight cone with 20 yd reach, plus omnidirectional hearing. An idle player is silent; actual grounded movement has a 2.5 yd walking, 6 yd running or 10 yd sprinting radius. Rolls, landings and weapon attacks carry 8 yd, spellcasting 12 yd, Thunder Clap 14 yd and Battle Shout 16 yd. Physical proximity within 1.25 yd also wakes an enemy. These are initial tuning values, shared by enemy families.

Both channels require an unobstructed world ray and at most 5 yd vertical separation. This first version treats a solid wall as soundproof, preserving the mine/ground separation. Actual motor velocity determines footstep noise; holding movement against a wall or flying through a jump does not produce footsteps. Hearing uses gameplay actions, independent of audio settings.

Once engaged, the existing combat memory, damage reaction and 30 yd / 8 yd height leash apply. Looking away never ends an ongoing fight. Wandering/database orientations remain authoritative.

A fixed hearing radius would not reward walking. Suspicion meters and searching would add a separate AI behavior beyond this request. Keep immediate detection and expose its cause in diagnostics for tuning.

Verification: boundary and noise unit tests; real NPC-loop tests for front/back and movement; solid collision test; full existing gates; isolated live front/back/movement scenarios with screenshots and logs.
