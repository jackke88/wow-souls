# Existing Spoken Player quest recordings

189 local `.ogg` recordings (18.30 MiB) for the 109 quest IDs in the active Elwynn manifest
were copied from the developer's existing SpokenQuestsAudioAlliance/Shared/Horde packs.
Only recordings already present were selected; no audio was generated or altered. Quest
IDs with no supplied recording remain text-only, as before. SHA256SUMS records each copy.

This preserves the existing voice feature while removing a hardcoded dependency on a
developer research directory. The original three-pack directory structure/search order
is retained. `WOWSOULS_SPOKEN=0` disables voices; an explicit path selects another pack root.

Source project: [rusty-key/spoken-wow](https://github.com/rusty-key/spoken-wow).
These recordings read Blizzard quest text. They are third-party content, not original WoW
Souls audio. The upstream [LICENSE](https://github.com/rusty-key/spoken-wow/blob/main/LICENSE)
licenses its own code under MIT; it explicitly excludes the game text and generated audio.
The [third-party notice](https://github.com/rusty-key/spoken-wow/blob/main/THIRD_PARTY.md)
identifies ElevenLabs/fish.audio and Blizzard terms for generated recordings. This directory
does not claim an MIT grant over the recordings or underlying quest text.
