# Landing contract

A landing on `main` is complete only when the `pages` workflow is green on that sha. It runs on push; if it is red, say so rather than re-triggering blindly. The deployed page is whatever `main` last built: https://bddap-bot.github.io/ziral/

`src/sim.rs` is the lockstep simulation and imports no Bevy type. Bevy stays in `src/main.rs`.

No code comments. A survivor states a why the code cannot show. Prose lives here, in the README, or in DESIGN.md.

`test-map.json` maps every touched path to the commands that must be green before landing.

The machine textures' critic (art/MACHINES.md) is a rule, not a dial: its rubric and threshold in `art/machines/manifest.toml` are never edited to raise a score. A change to either is a taste change, made in its own commit with its reason, never inside a paint round.
