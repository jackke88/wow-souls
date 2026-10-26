# wow-souls

Start: read `docs/HANDOVER.md` (about 8 KB) and nothing else. It is the whole bootstrap.

- Do NOT read `docs/archive/`, `docs/superpowers/plans/`, `README.md` or `docs/log/INDEX.txt` whole. Look things up:
  `grep -n -i <word> docs/log/INDEX.txt`, then `Read` the archive with `offset`/`limit`; module map `docs/ARCHITECTURE.md`; per-file `//!` docs.
- For broad searches use an Explore agent, not whole-file reads of big docs.
- HANDOVER.md holds state, not history: update "State" / "Next" in place, put the story of a step in `docs/log/<date>-<topic>.md`. Keep it under 8 KB.
- Gates and run commands are in HANDOVER. Never `cargo fmt --all`; build with `-j 4`; commit only when the user asks.
