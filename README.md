# ziral

A Bevy game. Design in progress: see DESIGN.md.

Toy 1 runs at https://bddap-bot.github.io/ziral/ and natively with `nix-shell --run 'cargo run'`.

Sessions record automatically in the shipped page. Open a record with the existing import action to replay it. The build SHA must match; keep the checkout for that SHA when analyzing an older record.

For native replay, run `cargo run -- --replay /path/to/record.json` from that checkout. Space pauses playback. G advances to the next recorded tick and S returns to the preceding tick; the camera remains free to pan and zoom.

M marks a moment without changing the game. `cargo run -- --analyze /path/to/record.json` prints marks and their thirty-second windows, with the frame times inside each.

Play in the web interface; recording needs no link or credential. The page creates a random session ID, retains pending inputs locally, and uploads every five seconds and seals a final chunk when the page is hidden or closed. Reloading retries retained records. `web/records-endpoint` fixes the public receiver node ID at build time; `ZIRAL_RECORDS_ENDPOINT` overrides it for a build. The browser transport and receiver both belong to ziral.

Install the receiver with `cargo install --path records --locked`. Run `ziral-records --key-file "$HOME/.config/ziral-records/key" serve --directory "$HOME/.local/state/ziral-records"`. `ziral-records --key-file "$HOME/.config/ziral-records/key" identity` prints its public node ID. Ban a session with `ziral-records ban <session-id> --directory "$HOME/.local/state/ziral-records"`. Sessions directly in the directory are bounded to 1024 files and 1 GiB; a new upload past either bound deletes the least recently updated sessions there, so move a record into a subdirectory to keep it. Banned sessions stay in place and do not count. Files have mode 600 beneath a mode-700 directory. Copy a record to the matching build for replay or analysis.

`cargo run -- --analyze record.json` prints first placement/bond/automated-recipe times, backpressure entries per minute, tape edits per arm, untouched palette rows, hesitation before each act, hover-card dwell, steps within each stall's ±15-second window, paused seconds, frame-time percentiles with the counts of frames longer than 1/60 s and longer than 1.5/60 s, and mark windows. Times use the recorded clock. Imported contents and preview steps do not count as milestone achievements. Tape-edit arm numbers survive relocation; importing a layout introduces new occurrences. A repeated blocked tick is still the same stall. Hesitation is time since the previous resolved act, not an inference about intent.

`nix-shell --run './web/build.sh && node web/bench.mjs'` runs the busy-scene frame benchmark on the native build and the web build; each passes when two consecutive thirty-second windows have no frame longer than 1/60 s. Other work on the machine lengthens its frames, so run it on an otherwise idle machine. `node web/bench.mjs https://bddap-bot.github.io/ziral/` measures the deployed page from a checkout at the deployed commit, since the analyzer reads only its own build's records. Opening the page at `#bench` runs the benchmark scene until reload; its autosave stays apart from the saved world, and its record is never uploaded.

Run the full test suite with `nix-shell --run 'cargo test --no-run && MALLOC_ARENA_MAX=2 cargo test -- --test-threads=2'`. Building first keeps the allocator limit off the compiler processes. The test run uses two threads and two glibc allocation arenas to reduce retained capture memory. Cargo builds at most six crates concurrently; the optimized test profile retains line-number debug information, debug assertions and overflow checks.

Render tests and the native benchmark share the deadline in
`web/render-timeout-seconds`. Run tests in `nix-shell` so the timeout debugger
is available. Each serialized render holds a cancellable watchdog through
teardown. On expiry it prints the test name and the watched thread’s GDB stack on Linux
(all threads on other platforms), then exits the test process unsuccessfully: a deadlocked
renderer cannot be safely reused by later tests. Stack collection has a
15-second kill limit. Successful renders cancel the watchdog immediately.
