# Soul binding at graveyards

Approved by Jacob on 2026-10-16: a memorial stone at each graveyard, with Falkhaan's tutorial.

- Derive graveyards from the manifest's non-event Spirit Healer spawns (entry 6491). Keep their database GUIDs as stable identifiers. The healer remains hidden; an authored stone marks the location.
- Interact with a stone using the existing Interact/Talk controls. Binding is free, explicit, unavailable while fighting, and replaces the character's active graveyard. Confirm the location and that death returns the player there.
- Save the active GUID in the character's own soulbind-v1.txt. Save successfully before publishing SoulBound or awarding tutorial progress. Preserve unreadable files. Existing characters without a binding retain their start point.
- On death retain existing bloodstain/currency behavior. Return the body beside the active stone and keep the death cover up until that destination's terrain and collision are resident. Starting the character again also uses the saved graveyard; explicit developer spawn overrides still win.
- Add a small runtime quest overlay, leaving imported DB quests intact: Falkhaan Isenstrider (6774), who gives Rest and Relaxation (2158), offers Bind Your Soul. Bind at a memorial, then return to Falkhaan. Count a real successful binding, including a binding saved before acceptance. Use the existing quest engine, tracker and rewards.
- Keep the core in soulbind.rs, stone presentation in soulbind/art.rs and the tutorial in quests/soul_tutorial.rs so the quest and animation chats can continue independently.

Verification: strict save parsing and isolation, DB-derived sites, range/height and combat refusal, switch/restart behavior, tutorial accept/bind/turn-in and prebound progress, ECS respawn with unavailable terrain, then unattended live bind/death/restart with isolated saves and a visual check of the stone.
