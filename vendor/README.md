These replace the Bevy 0.19.1, wgpu-hal 29.0.4 and glow 0.17.0 crates of the same names through `[patch.crates-io]`.
The crate sources and licenses come from their crates.io packages. Glow line endings and trailing whitespace are normalized.

| Crate | Published crate SHA-256 | Upstream commit |
| --- | --- | --- |
| `bevy_render` | `fc86a32b2eef9859e1e7d323c6d1a9150fe77aa17c30fd05abebc12f645929dc` | `b56fc29d3016e641754765244b5ba3f9cc504671` |
| `bevy_core_pipeline` | `2564f1ba766532e8eeb20059133ff6703e8dd5c3864b92378d0d1c541096ef12` | `b56fc29d3016e641754765244b5ba3f9cc504671` |
| `bevy_gizmos_render` | `2ea2b6266d210bca0aa3bcba7e17204e437b4e86cecd4021ab7dcd32da576a02` | `b56fc29d3016e641754765244b5ba3f9cc504671` |
| `wgpu-hal` | `97ace1c17727311c22a46e4e3faf56ea6de81af99dcc839bdfb54857b94d448d` | `e99f5305ded96ff7006f0714d043a7f735bd45c2` |
| `glow` | `29038e1c483364cc6bb3cf78feee1816002e127c331a1eec55a4d202b9e1adb5` | `0983efbc97fa4815f18990ef2a59540ede94f6f6` |

The changes are limited to:

- `bevy_gizmos_render/src/lib.rs`: update the previous GPU buffers, growing capacity when needed, instead of allocating four new buffers for each updated gizmo asset.
- `bevy_render/src/renderer/mod.rs` and `src/renderer/render_context.rs`: encode screenshots and readbacks before the graph's submission, using the existing submission helper and removing the second submission after the graph.
- `bevy_core_pipeline/src/lib.rs` and `src/schedule.rs`: queue uncovered-window clears before readbacks and remove the duplicate submission function.
- `wgpu-hal/src/gles/mod.rs`, `src/gles/fence_webgl.rs`, and `src/gles/queue.rs`: WebGL finishes each submission before advancing its completed value. It needs no pending sync objects or subsequent flush. Native GLES uses the original fence implementation. This trades GPU/CPU overlap for removal of per-frame WebGL sync allocations and their accumulated finalizers.
- `wgpu-hal/src/gles/framebuffer.rs`, `adapter.rs`, `command.rs`, `device.rs`, `mod.rs`, and `queue.rs`: GLES render passes and resolves reuse complete framebuffer attachment layouts. Keys include texture identity, mip/layer ranges, depth slice and sample count. Texture destruction evicts dependent layouts under the context lock; device and queue teardown clear the cache. Copy/readback operations retain their separate mutable framebuffer.
- `glow/src/web_sys.rs`: WebGL2 buffer, clear and uniform uploads reuse memory views and pass source offsets and lengths. The cached views refresh after memory growth; empty uploads do not turn into whole-memory uploads. Native GL and WebGL1 keep their original calls.

This gives each frame one submission and keeps the ordering of rendering, window clears, captures, and readbacks. The WebGL backend no longer creates an extra fence for an empty submission, and changing gizmo vertices does not discard their GPU storage.
