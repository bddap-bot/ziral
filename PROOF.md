# Hex bodies and registered seats

The machine pipeline owns topology. Every generator supplies material colour and surface variation; deterministic construction supplies hex plates, brass rims, rivets and circular seat apertures. Ming remains the default and its zero-spend guard is unchanged. The generator interface, bounded call-failure retries and worker limit are unchanged. There are no critic calls or geometric repaint rounds.

This deliberately changes the composition of the generated art. The previous detailed silhouettes cannot guarantee either the number of seats or their registration. They are replaced by consistent hex plates with generated material variation. An opaque material patch supplies mirrored surface grain; seatless machines retain the generated interior artwork beneath the hex rims. The comparison sheet exposes the visual tradeoff; branch review remains required before any main merge.

## Geometry contract

Each simulation footprint cell contributes a full hexagon. Their union is the only allowed body outline. Every interior pixel is opaque except an intentional seat aperture, and every exterior pixel is transparent. The same simulation cell coordinates place atoms, apertures and concentric seat rings. Body-only cells have no seats. Arms include their pivot and hand centres.

The encoded PNG is measured independently of construction. Its aperture centroid and ring-detector displacement must both be at most **6 asset pixels**. The asset scale is 256 pixels per 20-world-pixel hex radius, so this tolerance is **0.469 world pixels**, 0.938 screen pixels at the rig's two-screen-pixels-per-world-pixel scale. The detector's search grid is 5.12 asset pixels. Regression checks independently require analytic aperture centroids within one asset pixel. Missing holes or rims fail; they never trigger another model call. Bounds retain zero tolerance.

The regression fixture covers every machine with black, white and saturated generator images carrying an unrelated silhouette. It checks the entire raster against the exact hex union minus seat apertures, measures aperture centroids and runs the rim detector. Shipped assets are remeasured rather than trusting their recorded offsets.

## Reproduction and dependencies

```sh
nix-shell --run 'art/regenerate.sh --generator ming --jobs 3'
```

The command includes both release builds, all fresh model calls, deterministic construction, palette encoding, normal maps, articulated maps, the complete labelled sheet and the in-game rig screenshot. Existing dependencies remain: Rust/image for construction and measurement, Bevy for rendering, ImageMagick for packing and rig masks, pngquant for palette encoding, curl/jq for Ming transport and accounting, and coreutils timeout for external generators. No package or account was added. Silhouette boundary extraction, binary-search fitting and bilinear registration were deleted.

## Fresh full-set measurements

All 15 machines passed exact bounds on their first generated candidate. Thirty model calls cost $0 in the recorded ledger. Per-machine worst offsets below are the maximum of the encoded aperture centroid and image rim displacement.

| Machine | Outside pixels | Worst seat offset (asset px) | Generator wall time (s) |
|---|---:|---:|---:|

| arm-2 | 0 | 0.06 | 61.776 |
| arm | 0 | 0.05 | 63.771 |
| arm-3 | 0 | 0.09 | 71.099 |
| bonder | 0 | 0.05 | 69.329 |
| converter-cobalt | 0 | 0.04 | 90.715 |
| converter-amber | 0 | 0.04 | 111.126 |
| output-1 | 0 | 0.00 | 62.676 |
| output-2 | 0 | 0.00 | 76.762 |
| output-3 | 0 | 0.00 | 97.596 |
| portal | 0 | 0.00 | 87.549 |
| resonator | 0 | 0.05 | 66.831 |
| reification | 0 | 0.03 | 113.113 |
| second-bond | 0 | 0.04 | 100.331 |
| source | 0 | 0.05 | 78.343 |
| source-2 | 0 | 0.06 | 104.354 |

The visible composition changes are intentional: seated machines become hex assemblies with circular apertures; mirrored surface patches may repeat ornamental motifs. The portal and outputs preserve more of the generated interior illustration. These images are branch review evidence, not approval of the revised appearance.

## Complete-command timing

**Outer wall time: 894.604 seconds (14.91 minutes), exit 0.** The wrapper reports 888 seconds; the outer timer also includes entering the build environment. Release dependencies were already built. Every machine was freshly generated; no retained candidate or preview asset was reused. The wrapper's stages were:

| Stage | Wall seconds |
|---|---:|
| Initial release build | 64 |
| Generation, construction, maps and sheet | 531 |
| Embed updated art | 118 |
| In-game render, including GPU admission wait | 175 |

The renderer waited 160 seconds for a shared GPU reservation. Clippy ran concurrently during that wait; this was not an isolated performance benchmark. The previous complete run took 1,046.03 seconds (17.43 minutes). This run is 151.43 seconds shorter including its GPU wait, but host and model latency vary, so no stable speed multiplier is inferred. An earlier invocation lacked the generator credential and failed before making any model request; its 79.578 seconds are excluded from the successful run. The key was loaded from the existing account for this run.

The sheet and screenshot were both viewed. In the rig, the bonder's two atoms and second-bond's two occupied seats are centred in their apertures; the empty second-bond seat remains clearly visible. The source's atom also sits in its aperture. Every body follows the occupied hex cells. The comparison shows the previous sheet on the left and this run on the right, using the same machine order.

The renderer emitted an audio buffer-underrun warning, but no graphics error; it saved the screenshot and exited successfully. The regeneration wrapper verifies that the screenshot exists and fails on shader, pipeline, wgpu, naga or renderer errors.
