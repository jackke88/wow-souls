# Elwynn hidden treasures — approved design

User approval, 2026-10-16: expand the proposed twelve chests and ensure every class has a useful
equipment discovery. Implement eighteen Elwynn chests, in addition to the three Northshire caches.
The last user sentence clarifies coverage across the chests: each class must have a suitable chest;
each chest has one fixed reward bundle rather than dropping nine classes' equipment together.

Six caches contain usable Abbey Salves and fixed gems; four contain white equipment and eight
especially concealed caches contain guaranteed green equipment. Cloth Intellect/Stamina, leather
Agility, mail Strength, a dagger, a sword and a shield cover all nine Vanilla classes. Warrior and
Mage are playable now; the other classes remain planned, without enabling them as part of this work.
Use existing imported item rows with actual fixed stats, levels 3–10, unrestricted class masks.

Areas: Goldshire, Mirror Lake, Stonefield, Maclure, Crystal Lake, Jasperlode (two), Fargodeep (two),
Brackwell, Forest's Edge, Stone Cairn Lake (north fringe and east bank), Eastvale, Ridgepoint,
Jerod's Landing, Westbrook and Tower of Azora. Native DB chest sites can anchor concealed locations, with final
ground, approach and lid clearance checked in the game. No new terrain or mandatory jumps.

Retain Northshire IDs/rewards and the baseline chest logic. New caches have stable IDs, fixed
contents and permanent per-character claims using the existing atomic inventory transaction.
Closed chests have no marker or sparkle. Do not reset saves, change quests or edit installed games.
Document exact final coordinates, item IDs, quantities, required levels and class coverage.
