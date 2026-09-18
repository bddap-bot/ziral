# ziral

A Bevy game. Design in progress: see DESIGN.md.

Toy 1 runs at https://bddap-bot.github.io/ziral/ and natively with `nix-shell --run 'cargo run'`.

Sessions record automatically in the shipped page. Open a record with the existing import action to replay it. The build SHA must match; keep the checkout for that SHA when analyzing an older record.

For native replay, run `cargo run -- --replay /path/to/record.json` from that checkout. Space pauses playback. G advances to the next recorded tick and S returns to the preceding tick; the camera remains free to pan and zoom.

M marks a moment without changing the game. `cargo run -- --analyze /path/to/record.json` prints marks and their thirty-second windows.

A private relay installation issues a playtest link with `voice-web playtest-token`. Its fragment contains an opaque signed token and relay endpoint. The page keeps records locally, uploads every five seconds and on page exit or hiding, and retries retained records on the next load. The receiver uses the existing voice relay with a separate protocol and stores records under `~/.local/state/ziral-records/` with directory mode 700 and file mode 600. Copy a record privately to the matching build for replay or analysis. No token-to-person table is needed.
