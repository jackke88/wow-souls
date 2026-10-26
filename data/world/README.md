# Packaged world data

The default world is elwynn-westfall.txt. It combines the existing Elwynn/Northshire import
with Westfall, preserving spawn GUIDs, quest IDs, shared loot/vendor/item dependencies and
creature pool limits. Saves continue to identify creatures and graveyards by their old IDs.

The Westfall snapshot has 1,245 creature spawn records, 111 creature templates, 48 quests,
937 object spawns and 1,191 patrol points. The combined snapshot contains 3,194 creature sites
and 152 quests. Seasonal events and NYI records follow the existing base-world exclusions.
The Deadmines instance (map 36) is outside this outdoor map-0 import.

Rebuild from the existing exported database snapshot without starting the original server:

    cargo run -j 4 -p wow-world --offline -- import --zone westfall --out data/world/westfall.txt
    cargo run -j 4 -p wow-world --offline -- combine

The importer uses WoW AreaTable ancestry and terrain/interior floors, not just a coordinate
rectangle. Westfall has no Elwynn level-13 quest cap. Both zone imports must use the same
database snapshot. Re-exporting SQL uses tools/export-mangos.ps1 against a throwaway COPY.

For the full NPC roster, quest list and categorized animation/event work, see
[WESTFALL.md](../../docs/WESTFALL.md). Regenerate with wow-souls --audit-westfall and WOW_DATA.
New bodies use native client motion until custom kits are authored. Escort/event and spell
objectives remain incomplete until their world hooks exist; empty counters do not complete them.
