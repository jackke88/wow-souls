# 2026-10-28: the torch is only a light

The user: "ta bort det visuella ... när man equippar en torch så lyser det bara runt sig ... en action att kunna
släcka den så det inte tar från torchen ... aktiv i x många minuter innan torchen tar slut."

- **No model.** `WornKind::Belt`, `BeltTorch`, `BELT_TURN` (held.rs) and `torch::hang` / `orient` are gone; the
  torch item keeps its row, icon (display 12312) and shop shelf. A worn torch adds no `WornItems` entry.
- **Light.** `torch.rs::light` keeps one `point_light` + `FlameFlicker::Torch` + `HeldLight` entity, a child of the
  `Avatar` at `AT_HAND` (left, head height, a little ahead), while `Burning::glows()` (a torch in the slot and its
  flame up). Doused, off or spent: the entity is despawned.
- **Action.** `Action::Torch` ("Light / douse torch", World group, default `B`, no pad button: all sixteen are
  taken) in `settings/binds.rs`; the key-bindings page lists every `Action`, so it shows there and can be rebound.
  Ignored while a screen/modal is up or the player is dead. A torch put on is lit by itself.
- **Burn.** Only a lit, worn torch burns: `wow_world::torch::BURN_SECS` = 600 s (ten minutes of game time, the
  virtual clock, so a paused menu does not burn it). Spent = removed from the slot (`left` back to 0, the next torch is
  fresh). Toasts: "You light your torch. m:ss left." / "You douse your torch. m:ss left." / "Your torch burns out."
- **Save.** `torch-v1.txt` is now `wstorch 2` (`left`, `lit`); v1 files (no `lit` line) still load, as lit. Saved every
  5 s while burning and at once on a toggle; `lit` survives a restart, so a torch doused at quit stays out.
- **Debug switches.** `WOWSOULS_TORCH_AT=<s>[,<s>...]` presses the action at those real seconds (unattended runs ignore
  keys); `WOWSOULS_TORCH_SECS=<s>` shortens a full torch's burn to see one burn out. With `WOWSOULS_EQUIP=9100001` and
  `WOWSOULS_HOUR=22` a `WOWSOULS_RECORD` run shows it.
