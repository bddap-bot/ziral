# Housing coverage

[Before / after at native capture size](machine-housings-computed-67.png). Before assets: `51471f9fa6b9d27494031e4c97390d00ee823569`. Both sides use the same `housing:NAME` scene, default micro zoom, zero ticks and an unscaled 640 × 640 crop. Before captures precede every machine sprite change. All three arm asset directories remain byte-identical; the length-one arm is the visual control, with zero differing pixels between its before and after crops.

Ten glyph entries are present, including the second-bond applicator. Coverage is alpha-weighted sprite area divided by footprint area, using 256 capture pixels per hex circumradius. The figure includes the slight overshoot admitted by the unchanged footprint tolerance; it is not a shape-recognition score.

| Machine | Before coverage | After coverage | Kept candidate and critic |
|---|---:|---:|---|
| bonder | 29.2% | 83.9% | bonder-6 (7/10, no compound) |
| second-bond | 31.6% | 86.0% | second-bond-5 (7/10, no compound) |
| converter-amber | 22.1% | 83.4% | converter-amber-1 (7/10, no compound) |
| converter-cobalt | 46.3% | 89.3% | converter-cobalt-2 (7/10, no compound) |
| converter-plum | 29.1% | 89.9% | converter-plum-1 (6/10, no compound) |
| output-1 | 41.2% | 93.4% | output-1-3 (7/10, no compound) |
| output-2 | 43.0% | 92.2% | output-2-1 (6/10, no compound) |
| output-3 | 45.0% | 90.2% | output-3-7 (6/10, no compound) |
| reification | 43.0% | 87.6% | reification-4 (7/10, no compound) |
| source | 96.7% | 98.8% | source-7 (7/10, no compound) |
| arm | 22.5% | 22.5% | unchanged |

Critic scores below 8 are bounded three-round selections, not threshold changes. Every kept glyph passes the additional compound gate. The candidate histories, exact prompts, numerical gates and criticism remain beside the art. The critic's taste score is evidence, not a claim of perfect taste calibration.
