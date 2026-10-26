# Class design principles

Direction approved by the project owner on 2026-10-18. First implementation: Warrior, 2026-10-19.

**Preserve the WoW class fantasy and express it through Souls combat decisions: distance, timing, commitment, risk and resources. Talents develop how the player fights.**

- A weapon supplies reach, moves and basic tempo. A class supplies resources and characteristic tools. Talents reward deliberate ways of using those tools. Weapon changes remain meaningful within every specialization.
- Every specialization is a complete solo playstyle. Defensive mastery must create opportunities to win; offensive mastery must provide a sustainable way to survive. Enemy tells, hit volumes and commitment still matter.
- A major talent should answer: “When I do X, I receive Y, which makes me want to do Z.” Small numerical ranks support progression; noticeable behavior and signature abilities give a tree its identity.
- Progression adds bounded power and new opportunities for mastery. Evaluate gear, attributes, resources, talents and enemy growth together. Additional damage multipliers must not silently remove the need to read attacks.
- Trainers teach the basic class toolkit. Talent trees shape its use and unlock signatures. The equipped action set remains small enough for a controller: four ability slots in the current game.
- Keep recognizable specialization names and allow hybrids. Adapt individual mechanics for this single-player action game; MMO party-role requirements do not determine a specialization's usefulness.
- Base movement and timed defense are available independently of talents and keep their equipment restrictions. Talents improve rewards rather than purchase the ability to play skillfully.
- Resource/healing rewards require real contacts and have explicit per-action limits. Whiffs, damage-over-time pulses and crowd size must not accidentally create self-funding ability loops.
- Choices are inspectable, saved per character and reversible outside combat. Invalid saves never silently grant power or overwrite the original file.

## Warrior

A Warrior builds momentum through managing close combat and spends it on decisive actions. Stamina constrains the immediate action; rage supplies opportunities earned during the fight.

| Tree | Mastery | Signature in the first implementation |
|---|---|---|
| Arms | Read openings, commit to heavy blows, break defenses | Mortal Strike |
| Fury | Sustain offensive pressure, chain attacks and manage the point of retreat | Bloodthirst |
| Protection | Meet attacks, earn counters and take control of close combat | Shield Wall |

The first compact trees contain six five-rank talents and a signature each. Points start at level 10, with 51 available by level 60. Hybrids share this budget; only one signature fits. Implemented rules, defense timings and verification are recorded in `docs/log/2026-10-19-warrior-talents.md`.
