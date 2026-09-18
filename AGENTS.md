# Landing contract

A landing on `main` is complete only when the `pages` workflow is green on that sha. It runs on push; if it is red, say so rather than re-triggering blindly. The deployed page is whatever `main` last built: https://bddap-bot.github.io/ziral/

`src/sim.rs` is the lockstep simulation and imports no Bevy type. Bevy stays in `src/main.rs`.

No code comments. A survivor states a why the code cannot show. Prose lives here, in the README, or in DESIGN.md.

`test-map.json` maps every touched path to the commands that must be green before landing.

The machine textures' critic (art/MACHINES.md) is a rule, not a dial: its rubric and threshold in `art/machines/manifest.toml` are never edited to raise a score. A change to either is a taste change, made in its own commit with its reason, never inside a paint round.

## Boundaries

This ziral repository names only its own components. Name another project only as a declared, versioned dependency, never through its internals. Give a needed shared service a neutral name owned by this project. Do not import the environment of machines running agents: hostnames, addresses, paths outside the repository, service or queue names, credentials, camera frames, or renders of private places. No person's name, schedule or presence enters the repository. Before landing, grep the diff for other projects' names and host details. Remove host details and undeclared project references; dependency declarations expose only the dependency's name and version.
