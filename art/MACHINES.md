# Machine art pipeline

[BIBLE.md](BIBLE.md) supplies visual direction; each machine's `caption.txt` supplies its prompt. The manifest lists the 15 machines, call-attempt limit, seat-quality measurements and runtime rig metadata.

## Regenerate

Inside the repository, with the generator's credentials in the environment:

```sh
nix-shell --run 'art/regenerate.sh --generator ming --jobs 3'
```

This builds the tool, generates all 15 machines, constructs hex bodies and exact seat apertures from their materials, writes albedo and normal maps, splits articulated rig parts, assembles `art/machines/sheet.png`, rebuilds the embedded assets, and renders `proofs/machine-set-rig.png`. It prints each stage's elapsed time and the total. A failed machine or stage exits nonzero; a failed render is not reported as a completed set. The screenshot uses the game's headless renderer and requires a Vulkan driver, including a software driver if no GPU is available.

`ziral --gen --sheet` repacks the current sprites and diagnostic labels without generator calls.

`ziral --gen NAME... --generator ming --jobs 3` regenerates a subset. Every ordinary invocation generates fresh images. `--fit` instead reprocesses the retained sprites without generator calls; it cannot recover pixels already cropped from an old sprite. Symbols, pips, board tiles and the reference-based manual overlay retain their existing sources and are outside this machine command.

## Generator interface

Select `--generator ming` or `--generator ./art/my-generator.sh`. An external implementation is an executable called as:

```text
./art/my-generator.sh CAPTION_FILE OUTPUT_PNG
```

Read the UTF-8 caption file; write one square RGBA PNG cutout at the output path. It must contain both visible pixels and transparent background. Exit 0 on success, 3 for a policy refusal that must stop the run, or another nonzero status for a retryable failure. Stdout and stderr are saved beside the attempt as `generator.log`. Calls have a 900-second timeout. No downstream code changes are needed to try another implementation.

The Rust interface is equally small: caption path and attempt directory in, `Result<RgbaImage, Refusal>` out. The built-in Ming implementation lives in `src/machines/ming.rs`. It sends the caption to Design, sends that image to Design-Layer, and combines Design's colour with the object layer's alpha. Design cannot receive a reference image. Non-square, empty, opaque framing or incomplete layer responses are unusable generator results and fail the call. The shadow and background layers do not enter the sprite. Tiny alpha noise below 8/255 is removed inside this implementation.

Ming reads `OPENROUTER_API_KEY` from the environment. Its transport in `art/ming.sh` verifies zero endpoint price and zero account usage before calls, and verifies usage immediately after every request, including failures. A nonzero cost or unavailable accounting exits 3. A shared stop file prevents subsequent stages from starting after another worker observes a policy refusal; requests already in flight finish their accounting checks. The parent awaits every worker. Switching generators does not silently grant permission to incur new charges.

## Default deterministic downstream stages

1. **Material extraction and hex construction.** The generator supplies colour and fine surface variation, not topology. Sample its opaque pixels for the saturated material palette and choose an opaque surface patch for mirrored texture. Seatless machines retain their generated interior artwork beneath the hex rims. Construct every footprint cell as a full hex plate with a brass bevel and rivets. This replaces silhouette scaling: generated rhombi, octagons, extra holes and displaced rings cannot survive as body geometry. Ming remains the default, but the assembly changes its composition; compare the review sheets before a merge.
2. **Seats and exact bounds.** Construct circular apertures and concentric brass/glaze rings at the simulation's marked cell centres, including pivots and hands. No generated aperture is retained. Every pixel outside the exact hex union is transparent; every interior pixel is opaque except the intentional apertures. Check again after palette encoding. Bounds have zero tolerance.
3. **Measurement.** Independently run the image rim detector on the encoded sprite; fail the machine if its worst displacement exceeds 6 asset pixels. At 256 asset pixels per 20-world-pixel hex radius, this is 0.469 world pixels (less than one screen pixel at the normal rig scale). The detector searches on a 5.12 asset-pixel grid. Analytic aperture centroids are tested within one asset pixel, including adversarial generator colours and silhouettes. Missing detections report infinity. Measurements never cause another model call. The sheet labels bounds and measured offsets separately; actual game cells and atom centres stay fixed.
4. **Runtime maps.** Each worker finishes its own machine, including rig splitting, before taking another job. A shallow height approximation from alpha and 15% luminance variation, blurred by four pixels, yields `normal.png`. `art/machines/rig.sh` derives the game's base/moving albedo, normal and emissive maps. Static housings have no split parts.
5. **Review.** Pack the sheet and render the rig with the new embedded art.

There is no critic, judge, geometry repaint loop or best-candidate selection. Up to the manifest's `attempts` generator calls are allowed only when the call fails or returns an unusable image. The first usable image proceeds once through deterministic processing. A downstream failure is loud and names its machine; it never spends another model call. `--jobs` bounds concurrent machines from 1 to 16, default 3. Model diagnostics remain in ignored `attempts/` directories. `attempts.tsv` records index, geometry verdict, outside distance, seat contrast, offset in hex circumradii and generator seconds. The manifest records the selected generator, caption key, retained attempt and normal-map key.

## Dependencies

No new package is required. Rust and `image` implement geometry and pixels; Bevy renders the real game screenshot. `curl` and `jq` carry and validate Ming requests. ImageMagick packs the sheet and splits the runtime rig maps. pngquant keeps the established compact palette format. The external adapter uses the existing coreutils `timeout` to bound an arbitrary executable. The retained `paint.sh` serves reference-based assets and the opt-in image-conditioned machine adapter. Each dependency has a current consumer.

## Image-conditioned comparison

`--generator layout:./art/layout-paint.sh` opts into painting the supplied layout with the existing image tool. Other executables can use the same `layout:` prefix. The two positional arguments stay unchanged; `layout.png` is written beside `OUTPUT_PNG` before the call. It depicts the complete padded square canvas, exact footprint and simulation seats. The caption remains the machine's existing caption. The adapter records its augmented prompt and verified attachment hash through `paint.sh`.

For this opt-in path, uniform canvas resampling and exact footprint clipping replace material extraction and hex construction. The generated interior pixels, ornament and seat positions survive. No seat is moved or reconstructed. The conditioned path keeps RGBA pixels without repeated lossy palette encoding. The same aperture-centroid and rim detector reports offsets; missing apertures report infinity. Offsets above 6 asset pixels remain visible in the sheet and attempt ledger instead of aborting or triggering another paint. Bounds remain mandatory. This is a comparison mode, not a seat-quality guarantee. The default `ming` path retains its deterministic hex bodies and exact seats.

No package is added to the application. The image adapter uses the existing image tool, ImageMagick and pngquant. A local structural-conditioning experiment can implement the same executable interface without adding its inference stack to the game.

The four-machine comparison found stronger arm and bonder character from the image-tool adapter, but their seat offsets exceeded tolerance. The local ControlNet probe retained hex structure without transparent seat apertures. Neither is an exact-geometry replacement for the default. The `machine-layout` screenshot fixture shows the arm, bonder, reification housing and first output together, with atoms at the simulation's bonder and reification seats. Render it after embedding a chosen subset with `ziral --shot review.png machine-layout 8`.

## Key-colour Gemini comparison

`--generator layout:./art/gemini-key.sh` selects the paid `google/gemini-3-pro-image` adapter using `OPENROUTER_API_KEY`. It requests one square 1K PNG with the layout and caption, asking for uniform `#FF00FF` seat interiors. Pixels within 16 of each RGB channel become fully transparent before fitting. The layout supplies only an exterior mask: enclosed seats are never reconstructed. ImageMagick resizes that mask; the reference measurement used Pillow, so boundary resampling can differ slightly before the exact footprint clip.

The adapter uses the existing curl, jq, ImageMagick and coreutils dependencies. It records returned pixels, prompt and response metadata beside each attempt, including actual `usage.cost`. Request failures stop without paid retries. Selecting this adapter incurs charges; it has no automatic aggregate spending cap. Ming's default and zero-spend guard remain unchanged.

A four-machine single-attempt sample passed exact bounds and the 6 asset-pixel seat tolerance: arm 0.25 px, bonder 0.24 px, reification 0.68 px; output-1 is seatless. Actual charges totalled $0.546700 and model calls totalled 77.16 seconds (85.47 seconds including local processing and accounting). A linear projection for 15 machines is $2.05 and 289–320 seconds sequentially, excluding fitting, builds and rendering; three workers could reduce ideal model-call time to about 96 seconds, subject to provider throughput. This is a projection, not a full-set run or geometry guarantee.

Visual limitations remain: painted checkerboard inside the arm footprint, magenta seat fringes, and unwanted magenta decorations on the seatless output. The adapter is available for comparison; shipped assets and the default generator are unchanged.
