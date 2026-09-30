# Working on ziral

Edit by subtraction: resolve a problem by deleting code; a tactical patch over a symptom is not accepted. One implementation per thing, never two alive.

Delete code comments; keep only a why the code cannot show.

A landing on `main` completes when the `pages` workflow succeeds for that commit.
Report a failed workflow before retrying. [test-map.json](test-map.json) selects
covering checks.

Keep Bevy out of `src/sim.rs`; it is the lockstep simulation. Bevy belongs in
`src/main.rs`.

For machine textures, follow [art/MACHINES.md](art/MACHINES.md). Do not change the
rubric or threshold in [art/machines/manifest.toml](art/machines/manifest.toml) to
raise a score. Change taste criteria in a separate commit with a reason, outside
a paint round.

## Boundaries

Keep this project independent. Reference other projects only as declared, versioned
dependencies, exposing names and versions rather than internals. Give shared services
neutral project-owned names. Exclude deployment-specific paths, addresses, service
or queue names, credentials, camera frames and private renders. Before landing,
inspect the diff for undeclared project references and deployment details.
