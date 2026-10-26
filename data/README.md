# Packaged project data

`world/elwynn.txt` and `world/northshire.txt` are the active vMaNGOS world exports already used
by the game, copied from the former ignored development cache on 2026-10-17. They contain
spawn/quest/item rules and text; they are not WoW client archives. Their existing attribution
and licence status are recorded in `docs/PROVENANCE.md` (vMaNGOS/database and quest text).

The default runtime reads `data/world/elwynn.txt`. Updating an import is an explicit development
action; it must not silently select a scratch `.next` export. `WOWSOULS_WORLD_FILE` can select
a development manifest, and `WOWSOULS_DATA_DIR` can select a complete packaged data root.

`voices/` contains the 189 already-used Spoken Player quest recordings available for this
world, with their existing provenance and a SHA-256 inventory. Voice playback is optional;
`WOWSOULS_SPOKEN` can disable it or select another pack root. No private developer research
directory is searched by default. See `voices/README.md` for attribution and terms.
