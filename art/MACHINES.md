# Machine textures

[BIBLE.md](BIBLE.md) offers state colours, visual direction and inspiration. Each machine's `caption.txt` is its brief. [machines/manifest.toml](machines/manifest.toml) holds the attempt budget, the measured thresholds and each machine's rig, sound and particles. [src/machines.rs](../src/machines.rs) owns registration, measurement, selection and relief.

## Caption → Design → Design-Layer

A caption is a declarative art director's description of the finished sprite: subject, silhouette in hexagon terms, apertures, materials, ornament, light and what is absent. It draws on the bible for inspiration and adds no gates.

`art/ming.sh design` sends the caption unchanged to `inclusionai/ming-image-0.1-design` on OpenRouter, which returns one 2048 px square picture on a plain background. It takes no reference image, so the footprint reaches it only through the caption's words.

`art/ming.sh design-layer` sends that picture with one fixed layer plan to `inclusionai/ming-image-0.1-design-layer`: the complete object with its through-holes empty, the shadow beneath it, the background. It returns 1024 px RGBA layers. The object layer's alpha, scaled to the picture with values under 8/255 dropped as the layer's background noise, is the sprite's alpha; the colour is the picture's. The shadow and background are discarded: runtime lighting supplies direction through the normal map.

Both calls check the listed price before and the account usage before and after, a failed call included. A non-zero price, a non-zero reported cost or a usage change exits 3 and stops every painter in the run; any other failure is a failed attempt. `OPENROUTER_API_KEY` comes from the environment.

## Attempts

Inside `nix-shell`, `cargo run -- --gen NAME...` or `--gen --all`. Each machine gets up to `attempts` Design → Design-Layer runs, at most five in flight across machines, and stops at the first that passes:

1. **Matte.** A non-square picture or object layer, a split with one layer, an object layer covering under 3% or over 90% of its frame, or one that reaches the frame's edge fails the attempt.
2. **Fit.** The silhouette's bounding centre goes to the footprint centre and one uniform scale makes it the largest that stays inside the union of footprint hexes. Nothing is stretched or rotated.
3. **Register.** On a machine with seats, the seat rims found near their cells give one translation and one uniform scale by least squares.
4. **Measure.** `outside` is the farthest a visible pixel lies beyond the footprint, `seat` the rim contrast of the weakest seat, `off_centre` the farthest registered seat from its cell, all in hex circumradii against the manifest thresholds.

With no pass, the attempt whose worst measurement is closest to its threshold is kept and its failing verdict stays on the sheet. A run where no attempt made a sprite does not land.

`attempts.tsv` records every measured attempt: verdict, the three measurements, the two call times and the cost. The pictures and layers stay in the ignored `attempts/` directory. `painted` in the manifest keys the caption, the budget and the layer plan; a change repaints from attempt one.

## Relief

`albedo.png` is the registered sprite cropped to the machine's quad, 256 px per hex, quantised. A deterministic shallow height approximation combines alpha with 15% luminance variation, blurred by four pixels; central differences produce `normal.png`, keyed to the albedo by `relief` in the manifest. This is approximate relief, not recovered geometry; the shader lights it.

`art/machines/rig.sh` splits articulated machines into a central circular moving part and its base, with colour, normal and emissive maps; static housings keep an empty part list. `sheet.png` shows every kept sprite on clay with its attempt and verdict.

Directive: outputs do not need holes. An output is a space to place things — a magical acceptor, a table, or similar doodads — with Opus Magnum as the inspiration. No explicit acceptor hole for every atom.
