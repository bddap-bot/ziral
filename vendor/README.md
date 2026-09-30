These replace the Bevy 0.19.1 crates of the same names through `[patch.crates-io]`.
The crate sources and licenses come from their crates.io packages.

| Crate | Published crate SHA-256 | Upstream commit |
| --- | --- | --- |
| `bevy_render` | `fc86a32b2eef9859e1e7d323c6d1a9150fe77aa17c30fd05abebc12f645929dc` | `b56fc29d3016e641754765244b5ba3f9cc504671` |
| `bevy_core_pipeline` | `2564f1ba766532e8eeb20059133ff6703e8dd5c3864b92378d0d1c541096ef12` | `b56fc29d3016e641754765244b5ba3f9cc504671` |
| `bevy_gizmos_render` | `2ea2b6266d210bca0aa3bcba7e17204e437b4e86cecd4021ab7dcd32da576a02` | `b56fc29d3016e641754765244b5ba3f9cc504671` |

The changes are limited to:

- `bevy_gizmos_render/src/lib.rs`: update the previous GPU buffers, growing capacity when needed, instead of allocating four new buffers for each updated gizmo asset.
- `bevy_render/src/renderer/mod.rs` and `src/renderer/render_context.rs`: encode screenshots and readbacks before the graph's submission, using the existing submission helper and removing the second submission after the graph.
- `bevy_core_pipeline/src/lib.rs` and `src/schedule.rs`: queue uncovered-window clears before readbacks and remove the duplicate submission function.

This gives each frame one submission and keeps the ordering of rendering, window clears, captures, and readbacks. The WebGL backend no longer creates an extra fence for an empty submission, and changing gizmo vertices does not discard their GPU storage.
