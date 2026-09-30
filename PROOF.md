# Art pipeline: full-set regeneration and guaranteed hex bounds

The branch replaces geometry retry rounds with a generator interface followed by deterministic fitting, exact hex clipping and runtime-map production. The default remains Ming, with the same captions and visual direction. This is a branch-only pipeline improvement; main is unchanged and no merge is part of this result.

```sh
nix-shell --run 'art/regenerate.sh --generator ming --jobs 3'
```

The command regenerates all 15 machine sprites, albedo/normal/rig maps, the [contact sheet](art/machines/sheet.png), and the [in-game rig screenshot](proofs/machine-set-rig.png). A custom generator uses `--generator ./path/to/executable`: it receives `CAPTION_FILE OUTPUT_PNG` and returns a square RGBA cutout. Exit 3 stops on policy refusal; ordinary call failures have the manifest's bounded retry budget. All downstream geometry is shared. [The interface and stage contracts](art/MACHINES.md) include the exact command and failure behavior.

## Bounds and seats

Every nonzero-alpha edge participates in body fitting. Pixels outside the exact union of footprint hexes are zeroed, including a post-quantisation check. The former least-squares seat transform and configurable outside tolerance are deleted. A successful generator image is processed once; weak seats do not cause another model call. Body containment wins when seat alignment conflicts with it.

The sheet labels bounds separately from seat quality. Offsets below are the worst detected seat displacement per machine at 256 asset pixels per hex circumradius. The rim detector searches locally and resolves positions on a 5.12 px grid; these are image estimates rather than semantic matching of apertures. Outputs and portal have no seats. Game cell and atom positions are unchanged.

All 15 freshly generated machines have zero outside pixels.

| Machine | Bounds | Worst detected seat offset (px) | Generator wall time (s) |
|---|---|---:|---:|
| arm | pass, zero overflow | 76.80 | 76.708 |
| arm-2 | pass, zero overflow | 110.29 | 74.736 |
| arm-3 | pass, zero overflow | 62.29 | 78.121 |
| bonder | pass, zero overflow | 112.64 | 100.628 |
| converter-amber | pass, zero overflow | 112.64 | 88.293 |
| converter-cobalt | pass, zero overflow | 103.55 | 101.547 |
| output-1 | pass, zero overflow | 0.00 | 74.126 |
| output-2 | pass, zero overflow | 0.00 | 69.306 |
| output-3 | pass, zero overflow | 0.00 | 80.295 |
| portal | pass, zero overflow | 0.00 | 81.105 |
| reification | pass, zero overflow | 133.12 | 94.171 |
| resonator | pass, zero overflow | 66.56 | 70.191 |
| second-bond | pass, zero overflow | 94.13 | 108.251 |
| source | pass, zero overflow | 56.32 | 116.818 |
| source-2 | pass, zero overflow | 100.85 | 82.989 |

## Simplification, measured against main

Baseline is main at `7b143b9`. Counts are physical lines including blanks and tests, from `git show 7b143b9:PATH` and the branch files. The listed machine-pipeline code and transport fixtures shrink from **4,169 to 2,380 lines: 1,789 removed net (42.9%)**. The Rust production portions before their test modules shrink from 2,358 to 1,267 lines; this is not just deleted tests.

| File | Main | This branch |
|---|---:|---:|
| `src/machines.rs` | 3907 | 1771 |
| `src/machines/ming.rs` | 0 | 243 |
| `art/direct.sh` | 16 | 0 |
| `art/ask.sh` | 48 | 0 |
| `art/tests/ask.sh` | 64 | 0 |
| `art/paint.sh` | 103 | 103 |
| `art/ming.sh` | 0 | 108 |
| `art/tests/ming.sh` | 0 | 99 |
| `art/regenerate.sh` | 0 | 25 |
| `art/machines/rig.sh` | 31 | 31 |

Six removed operations account for the simplification: scaffold/reference raster preparation for the machine painter; director/rebrief calls; critic calls and ranked selection; chroma-key removal; palette diagnostics; and calibration relight rendering/measurement. The geometry-driven repaint loop added by the initial Ming experiment is also gone. The replacement has caption → generator RGBA → body fit/hex clip → normals/rig maps → sheet/real-game render. Ming internally needs two model calls to supply RGBA; another implementation can supply it in one. Seat diagnostics remain informational.

The retained `paint.sh` serves reference-based tiles and manual-page art, so deleting it would break a live consumer. Normals and rig maps are loaded by the game and remain necessary. No package was added by this change: curl/jq already support the branch's Ming transport, ImageMagick packs and splits, pngquant encodes palettes, and the existing image/Bevy code handles pixels and rendering. The initial Ming branch had explicitly added curl and jq to the Nix environment.

## Speed, measured end to end

The complete command ran on one shared host with three bounded workers and fresh generation for every machine. Its outer timer includes Nix entry, both release builds, all API calls and credit checks, fitting, clipping, normals, rig splitting, sheet packing and the real game screenshot. Dependencies were already built. Each worker finishes its own rig maps, avoiding a serial rig-splitting tail.

**Full 15-machine command: 1046.03 seconds (17.43 minutes), exit 0.**

| Stage | Wall time |
|---|---:|
| initial-build | 45 s |
| generation-and-packing | 785 s |
| embed-updated-art | 189 s |
| render | 21 s |

The fixed-subset comparison regenerated `portal` on each pipeline on the same host, with release binaries built beforehand. Main used `ziral --gen portal`; this branch used `ziral --gen portal --generator ming --jobs 1`. The original main caption and candidate cache were invalidated. Neither run reused old generated candidates. These times include caption preparation where applicable, API calls, fitting and measurement, albedo/normals, main's calibration renders, and each pipeline's normal whole-set review sheet (candidate grids on main, 15 retained sprites here). They exclude compilation and the in-game screenshot on both sides. Main retained portal-7 after 12 paints across all three critic rounds; its capped critic score was 7. A full main repaint was avoided because of that call count and duration.

| Fixed subset | Wall time | Exit |
|---|---:|---:|
| Main, portal | 593.79 s | 0 |
| New pipeline, portal | 96.08 s | 0 |

The new pipeline was **6.18× faster on this measured subset**. The full new set is measured above; no full-set speed multiplier is inferred. These are single wall-clock samples under shared-host load, not an isolated throughput benchmark. The subset benchmark's temporary portal was archived separately, then the complete command's original assets and sheet were restored; the review images show that full-set run.

## Cost, validation and limitations

The final run's per-call ledger and before/after account checks record $0: the zero-spend guard remains in force. No new account or paid signup was used. The default model and caption set remain Ming, so no alternative-style comparison is claimed.

The regression suite covers every alpha level against every footprint, exact containment of every shipped albedo, matching normals/rig maps, a replacement executable generator, call-failure retry limits, policy refusal, no geometry/critic retries, and no generator retry after a downstream rig failure. Offline shell fixtures cover unavailable/nonzero credit usage, changed pricing, invalid costs, failed requests, malformed image data and the shared parallel stop signal. The landing map additionally requires formatting, clippy with warnings denied, the full Rust suite, native/web release builds and the native/browser frame benchmark.

An initial full run produced all art and a screenshot but returned 1 because the wrapper misclassified audio-buffer errors as graphics failures. The wrapper now checks graphics errors specifically and verifies the screenshot exists; the full command was rerun for the successful measurements and images above. Long sheet labels were also shortened before that rerun. These were pipeline defects fixed during validation, not artistic retry rounds.

Bounds are guaranteed; semantic seat placement is not. Detected offsets and low rim contrast stay visible in the records, with containment taking priority. Strong painted shading remains part of the approved Ming look. The earlier [lighting-split probe](proofs/ming-light-split-162.png) separated objects rather than clean lighting terms, so it is not a production stage. Symbols, pips, atoms, bonds, board tiles and the reference-based manual page retain their existing assets and workflows. Main and its deployment are unchanged; these images are for branch review before any merge.
