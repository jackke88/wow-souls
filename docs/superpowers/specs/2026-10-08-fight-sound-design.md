# Fight sound: design (sub-project D, part 1)

Date: 2026-10-08. D (impact) is split: this is D1, sound. D2 (hit-stop by weight, camera shake, knock-back
from the params, sparks and blood, the player's Dark Souls reactions and poise) keeps its own spec.

## Goal

The fight is audible, with WoW's own sounds from the user's install: the sword's swish, the blade on flesh,
the kobold's voice (aggro bark, attack grunt, wound, death) and its blow landing on the player. The user said:
"Dark Souls sounds for the hit, I think? Or maybe WoW's own works. You decide." Decision: WoW's now.

Why not Dark Souls' first: its sounds are FMOD banks (`sound/*.fsb`, FSB4, with `.fev` event files) mapped
through TAE sound events and sound ids: an FSB4 reader, the codec behind it and the event map are a separate
extractor job (GPL, local only, like the clips). WoW's sounds need none of it: the catalogs are in
`benilla-formats` (`SoundEntries`, `WeaponImpactSounds`, `WeaponSwingSounds2`, `CreatureSoundData`,
`ItemSubClass`) and the files are plain `.wav` in the MPQs. Everything goes through a kit id and a
position, so a Dark Souls bank can answer the same call later.

## Success criteria

1. Triggers, each one logged in a null run (`WOWSOULS_AUDIO=null`, full path with the output discarded):
   the player's swing plays the swing kit of its weapon weight just before the hit window; a landed hit plays
   the sword's impact kit for the victim's material (the kobold: flesh) at the hit point and the kobold's
   wound vocal (60 % of hits) or its death cry (a kill); the kobold's aggro bark when it notices the player;
   its attack grunt (70 %) at the telegraph and its swish before its hit window; its blow on the player plays
   the unarmed flesh impact at the player (the kobold's `CreatureSoundData` row has no `CustomAttack` column:
   its attack grunt is the `exertion` kit, `KoboldAttack` A-C).
2. Every file of every kit the fight uses decodes (install test, `WOW_DATA`).
3. The mixer's arithmetic is unit-tested without a device: sum, soft limit, pitch, end of sound, pan, voice cap.
4. One real run opens the output device and plays without an error (a short smoke run at low volume); the
   unattended runs are silent unless asked (`WOWSOULS_AUDIO=1` or `null`).
5. The user judges: the volumes, which sound for which event, how it reads against the fight.

## Non-goals here

Dark Souls sounds, footsteps, music and ambience, spell sounds, reverb (EAX), the player's own voice (the
character vocal tables), UI sounds, occlusion and doppler, streaming. Camera shake, hit-stop tuning,
knock-back and effects are D2.

## Facts (measured 2026-10-08)

- The install holds 332 weapon sound files (`Sound\Item\Weapons\...`) and the kobold's voice (`Sound\Creature\
  Kobold\`: aggro, attack A-C, death, fidgets); the four tables above are in `DBFilesClient`.
- benilla-formats loads each table (`load_sound_kit_catalog`, `load_weapon_impact_catalog`,
  `load_weapon_swing_catalog`, `load_creature_voice_catalog`, `load_item_sub_classes`); `Catalogs.chain` is the
  open patch chain. benilla-app's own mixer (kira, a patched copy that needs cpal 0.18 and its own output
  backend, 15 000 lines tied to the network client) is not portable; its rules are (below).
- Rules taken from benilla-app's `sound/combat.rs` and `kit.rs` (reference-checked there): a creature victim's
  impact slot is its `CreatureSoundData.impact_type` mapped {0 flesh, 1 stone, 2 wood, 3 ethereal}; metal
  weapons use the metal row of `WeaponImpactSounds`; a weaponless or natural-weapon attacker plays its
  `CustomAttack` column, else the unarmed row (subclass 13); the swish is `WeaponSwingSounds2` by the weapon's
  `ItemSubClass.WeaponSwingSize` (0 light, 1 medium, 2 heavy); exertion chance 70 % (creature) / 35 %
  (player), injury 60 % / 30 %; a kit's variations are weighted by `Freq`; a kit's `Volume`, `MinDistance`
  and `DistanceCutoff` shape the gain. The starting sword is Sword1H (subclass 7, metal).
- `cpal` 0.17 and `symphonia` 0.6 (`wav`, `pcm`, `adpcm`) are in the local cargo cache and resolve offline;
  `kira` is not an option (see above).

## Architecture

```
crates/wow-souls/src/
  sfx.rs      pure: decode a .wav to stereo at the device rate (symphonia, linear resample), pick a
              variation by weight, the gain and pan of a source, the impact slot, the chance roll
  mixer.rs    pure `Mixer` (voices, fill, soft limit, voice cap) + `Output`: a thread that owns the cpal
              stream and the mixer behind a channel; `Output::open()`
  audio.rs    the Bevy side: `Audio` resource (output, catalogs, decoded cache, dice, volume, mode), the
              triggers (systems reading `Hit`, `Telegraph`, `PlayerHurt`, `Action`, `Brain`, `Hostile`)
```
New dependencies (workspace): `cpal = "0.17"`, `symphonia = 0.6` (`wav`, `pcm`, `adpcm`).

- **Mixer.** A voice is a decoded sound (`Arc<[[f32; 2]]>` at the device rate), a fractional position, a pitch
  step and a left and right gain. `fill` adds every voice into the buffer, retires finished ones and clamps the
  sum to [-1, 1] (a handful of voices at a 0.6 master gain never reaches it).
  At most 32 voices; a new one past the cap is dropped. The stream callback drains a channel of new voices
  (`try_recv`, no allocation beyond the voice list's reserved capacity) and calls `fill`.
- **Output thread.** `cpal::Stream` is not `Send` everywhere, so a thread opens the default output device,
  builds the stream (f32, else i16 converted), reports the device rate back, and parks until the `Output` is
  dropped. The `Audio` resource holds only the channel's sender.
- **Sounds.** A kit's variation is picked by weight, read once from the chain (first use or preload),
  decoded to the device rate, cached by path (a miss is cached as a miss, warned once).
- **Space.** The listener is the game camera. A source's gain: kit volume x master x (1 inside the kit's
  `MinDistance`, linear to 0 at its `DistanceCutoff`; a cutoff of 0 is non-positional); pan: the source's
  direction against the camera's right, constant power.
- **Triggers.** Everything reads what the game already produces: `Hit` (player's blow landed: impact kit,
  wound or death vocal), `Telegraph` (kobold attack: exertion), `PlayerHurt` (kobold's blow landed: its
  `CustomAttack` kit at the player), the avatar's `Action` (a swing's time crossing the hit window start minus
  0.08 s: the swish), the kobold's `Brain` (its swish before its hit window), `Hostile.alert` rising (aggro).
  The kobold's voice row is found through a `VoiceOf(display id)` component set at its spawn.
- **Modes.** `WOWSOULS_AUDIO=0` off, `null` the whole path with the output discarded and every play logged,
  `1` the device even in an unattended run; unset: the device in an attended run, off in an unattended one.
  `WOWSOULS_VOLUME=<0..1>` master, default 0.6. `WOWSOULS_TRACE` logs each play in device mode too.

## Error handling

A missing table, a device that cannot open, a file that does not decode: one warning, the game plays on
silently. A sound is never a reason to stop.

## Testing

`sfx`: weighted pick (weights, all-zero weights, seeded), gain curve (inside, edge, beyond, non-positional),
pan, chance roll, slot map, decode of a synthesized PCM16 mono and stereo wav (length, resample ratio, mono
to both channels, a truncated file is an error). `mixer`: sum of two voices, the limit, pitch halves the
length, a voice ends and is retired, the cap, panned gains. Install (`WOW_DATA`): the fight's kits decode;
the kit ids the triggers use are non-zero. Live: a null run through the fight, one log line per trigger; one
device smoke run.

## Locality

WoW's sound files stay in the MPQs; nothing is copied out. The decoded cache lives in memory.

## Risks

- **Mix.** WoW's kits are mastered for a different mix (spells and ambience around them); the master volume
  and per-kind scales are constants to tune by ear.
- **Timing.** The swish is placed from the hit window, not from an animation event (the retargeted clips carry
  none): 0.08 s before the window opens; tune by ear.
- **Device.** WASAPI shared mode at 48 kHz is expected; an exclusive or odd format falls back to silence with
  a warning.
