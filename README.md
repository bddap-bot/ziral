# ziral

A Bevy game. Design in progress: see DESIGN.md.

Toy 1 runs at https://bddap-bot.github.io/ziral/ and natively with `nix-shell --run 'cargo run'`.

Sessions record automatically in the shipped page. Open a record with the existing import action to replay it. The build SHA must match; keep the checkout for that SHA when analyzing an older record.

For native replay, run `cargo run -- --replay /path/to/record.json` from that checkout. Space pauses playback. G advances to the next recorded tick and S returns to the preceding tick; the camera remains free to pan and zoom.

M marks a moment without changing the game. `cargo run -- --analyze /path/to/record.json` prints marks and their thirty-second windows.
