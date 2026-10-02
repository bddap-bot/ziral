# Working on ziral

Edit by subtraction: resolve a problem by deleting code; a tactical patch over a symptom is not accepted. One implementation per thing, never two alive.

Delete code comments; keep only a why the code cannot show.

A landing on `main` completes when the `pages` workflow succeeds for that commit.
Report a failed workflow before retrying. [test-map.json](test-map.json) selects
covering checks.

Keep Bevy out of `src/sim.rs`; it is the lockstep simulation. Bevy belongs in
`src/main.rs`.

`nix-shell --run 'cargo run --release -- --paint [--generator CMD] [MACHINE...]'`
regenerates machine sprites. A generator is any command run as `CMD OUT.png SIDE
LAYOUT.png [SIBLING.png]` with the prompt on stdin; it writes a square image of the layout repainted
and prints its cost in USD. `art/paint/openrouter.sh MODEL` reaches every OpenRouter
image model through `OPENROUTER_API_KEY`; `art/paint/layout.sh` returns the layout
itself. The command keys out flat green, masks each sprite to its hex footprint,
rejects an off-aspect return or a painted surround, and writes
`art/machines/<name>/albedo.png`. Keep each caption in
[art/machines/manifest.toml](art/machines/manifest.toml) a declarative picture of a few
dozen words; machine sprites carry no writing. Show the new sprites and in-game
renders before new machine art lands on `main`.

## Boundaries

Keep this project independent. Reference other projects only as declared, versioned
dependencies, exposing names and versions rather than internals. Give shared services
neutral project-owned names. Exclude deployment-specific paths, addresses, service
or queue names, credentials, camera frames and private renders. Before landing,
inspect the diff for undeclared project references and deployment details.

Every landing runs the deterministic rendered gallery and its mechanical gates in `cargo test`; the manifest retains the complete native suite where its covering rule requires it. If `gallery/hashes.txt` differs from `.gallery/test/hashes.txt`, run `nix-shell --run 'gallery/critic.sh .gallery/test'`, resolve every changed scene below eight, and copy `.gallery/test/approved-hashes.txt` to `gallery/hashes.txt` in the same change. Repeat the tests after updating the hashes. Quote only changed scenes and their scores in the landing comment; with no changed frames, use `gallery unchanged`. Never invoke the critic for unchanged frames. Judgements persist in ignored `.gallery/reviews`, keyed by scene content and the fixed rubric, so retries do not pay again for unchanged evidence.
