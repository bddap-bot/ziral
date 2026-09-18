# Housing coverage and computed-light calibration

[Before / after at native capture size](machine-housings-computed-67.png). Before assets: `51471f9fa6b9d27494031e4c97390d00ee823569`. Both sides use the same `housing:NAME` scene, default micro zoom, zero ticks and an unscaled 640 × 640 crop. Before captures precede every machine and rig PNG change. All three arm asset directories remain byte-identical; the length-one arm is the visual control, with zero differing pixels between its before and after crops.

Ten glyph entries are present, including the second-bond applicator. Coverage is alpha-weighted sprite area divided by footprint area, using 256 capture pixels per hex circumradius. The figure includes the slight overshoot admitted by the unchanged footprint tolerance; it is not a shape-recognition score.

| Machine | Before coverage | After coverage | Kept candidate and critic | Maximum direction error | Sphere-shape error |
|---|---:|---:|---|---:|---:|
| bonder | 29.2% | 83.9% | bonder-6 (7/10, no compound) | 0.020° | 7.094° |
| second-bond | 31.6% | 86.0% | second-bond-5 (7/10, no compound) | 0.020° | 7.094° |
| converter-amber | 22.1% | 83.4% | converter-amber-1 (7/10, no compound) | 0.020° | 7.094° |
| converter-cobalt | 46.3% | 89.3% | converter-cobalt-2 (7/10, no compound) | 0.020° | 7.094° |
| converter-plum | 29.1% | 89.9% | converter-plum-1 (6/10, no compound) | 0.020° | 7.094° |
| output-1 | 41.2% | 93.4% | output-1-3 (7/10, no compound) | 0.020° | 7.094° |
| output-2 | 43.0% | 92.2% | output-2-1 (6/10, no compound) | 0.020° | 7.094° |
| output-3 | 45.0% | 90.2% | output-3-7 (6/10, no compound) | 0.020° | 7.094° |
| reification | 43.0% | 87.6% | reification-4 (7/10, no compound) | 0.020° | 7.094° |
| source | 96.7% | 98.8% | source-7 (7/10, no compound) | 0.020° | 7.094° |
| arm | 22.5% | 22.5% | unchanged | — | — |

Critic scores below 8 are bounded three-round selections, not threshold changes. Every kept glyph passes the additional compound gate. The candidate histories, exact prompts, numerical gates and criticism remain beside the art. The critic's taste score is evidence, not a claim of perfect taste calibration.

The generator supplies albedo only. Shallow normals approximate relief from colour and silhouette; they are not physical surface recovery. Each calibration image shades machine and sphere through one renderer with explicit light vectors. These directions are measured from the resulting pixels; the unchanged direction and sphere-shape limits are both 25°. The shipped-asset test checks every saved relight pixel against the same computed render before independently remeasuring calibration.

Angles below are azimuth / elevation. Azimuth is counterclockwise from image right in a coordinate system whose positive y points upward.

| Machine | Facing | Requested | Measured | Direction error |
|---|---|---:|---:|---:|
| bonder | upper-left | 120.000° / 45.000° | 120.006° / 44.989° | 0.020° |
| bonder | upper-right | 60.000° / 45.000° | 59.994° / 44.989° | 0.020° |
| bonder | right | 0.000° / 45.000° | 0.000° / 44.995° | 0.020° |
| bonder | lower-right | 300.000° / 45.000° | 300.006° / 44.989° | 0.020° |
| bonder | lower-left | 240.000° / 45.000° | 239.994° / 44.989° | 0.020° |
| bonder | left | 180.000° / 45.000° | 180.000° / 44.995° | 0.020° |
| second-bond | upper-left | 120.000° / 45.000° | 120.006° / 44.989° | 0.020° |
| second-bond | upper-right | 60.000° / 45.000° | 59.994° / 44.989° | 0.020° |
| second-bond | right | 0.000° / 45.000° | 0.000° / 44.995° | 0.020° |
| second-bond | lower-right | 300.000° / 45.000° | 300.006° / 44.989° | 0.020° |
| second-bond | lower-left | 240.000° / 45.000° | 239.994° / 44.989° | 0.020° |
| second-bond | left | 180.000° / 45.000° | 180.000° / 44.995° | 0.020° |
| converter-amber | upper-left | 120.000° / 45.000° | 120.006° / 44.989° | 0.020° |
| converter-amber | upper-right | 60.000° / 45.000° | 59.994° / 44.989° | 0.020° |
| converter-amber | right | 0.000° / 45.000° | 0.000° / 44.995° | 0.020° |
| converter-amber | lower-right | 300.000° / 45.000° | 300.006° / 44.989° | 0.020° |
| converter-amber | lower-left | 240.000° / 45.000° | 239.994° / 44.989° | 0.020° |
| converter-amber | left | 180.000° / 45.000° | 180.000° / 44.995° | 0.020° |
| converter-cobalt | upper-left | 120.000° / 45.000° | 120.006° / 44.989° | 0.020° |
| converter-cobalt | upper-right | 60.000° / 45.000° | 59.994° / 44.989° | 0.020° |
| converter-cobalt | right | 0.000° / 45.000° | 0.000° / 44.995° | 0.020° |
| converter-cobalt | lower-right | 300.000° / 45.000° | 300.006° / 44.989° | 0.020° |
| converter-cobalt | lower-left | 240.000° / 45.000° | 239.994° / 44.989° | 0.020° |
| converter-cobalt | left | 180.000° / 45.000° | 180.000° / 44.995° | 0.020° |
| converter-plum | upper-left | 120.000° / 45.000° | 120.006° / 44.989° | 0.020° |
| converter-plum | upper-right | 60.000° / 45.000° | 59.994° / 44.989° | 0.020° |
| converter-plum | right | 0.000° / 45.000° | 0.000° / 44.995° | 0.020° |
| converter-plum | lower-right | 300.000° / 45.000° | 300.006° / 44.989° | 0.020° |
| converter-plum | lower-left | 240.000° / 45.000° | 239.994° / 44.989° | 0.020° |
| converter-plum | left | 180.000° / 45.000° | 180.000° / 44.995° | 0.020° |
| output-1 | upper-left | 120.000° / 45.000° | 120.006° / 44.989° | 0.020° |
| output-1 | upper-right | 60.000° / 45.000° | 59.994° / 44.989° | 0.020° |
| output-1 | right | 0.000° / 45.000° | 0.000° / 44.995° | 0.020° |
| output-1 | lower-right | 300.000° / 45.000° | 300.006° / 44.989° | 0.020° |
| output-1 | lower-left | 240.000° / 45.000° | 239.994° / 44.989° | 0.020° |
| output-1 | left | 180.000° / 45.000° | 180.000° / 44.995° | 0.020° |
| output-2 | upper-left | 120.000° / 45.000° | 120.006° / 44.989° | 0.020° |
| output-2 | upper-right | 60.000° / 45.000° | 59.994° / 44.989° | 0.020° |
| output-2 | right | 0.000° / 45.000° | 0.000° / 44.995° | 0.020° |
| output-2 | lower-right | 300.000° / 45.000° | 300.006° / 44.989° | 0.020° |
| output-2 | lower-left | 240.000° / 45.000° | 239.994° / 44.989° | 0.020° |
| output-2 | left | 180.000° / 45.000° | 180.000° / 44.995° | 0.020° |
| output-3 | upper-left | 120.000° / 45.000° | 120.006° / 44.989° | 0.020° |
| output-3 | upper-right | 60.000° / 45.000° | 59.994° / 44.989° | 0.020° |
| output-3 | right | 0.000° / 45.000° | 0.000° / 44.995° | 0.020° |
| output-3 | lower-right | 300.000° / 45.000° | 300.006° / 44.989° | 0.020° |
| output-3 | lower-left | 240.000° / 45.000° | 239.994° / 44.989° | 0.020° |
| output-3 | left | 180.000° / 45.000° | 180.000° / 44.995° | 0.020° |
| reification | upper-left | 120.000° / 45.000° | 120.006° / 44.989° | 0.020° |
| reification | upper-right | 60.000° / 45.000° | 59.994° / 44.989° | 0.020° |
| reification | right | 0.000° / 45.000° | 0.000° / 44.995° | 0.020° |
| reification | lower-right | 300.000° / 45.000° | 300.006° / 44.989° | 0.020° |
| reification | lower-left | 240.000° / 45.000° | 239.994° / 44.989° | 0.020° |
| reification | left | 180.000° / 45.000° | 180.000° / 44.995° | 0.020° |
| source | upper-left | 120.000° / 45.000° | 120.006° / 44.989° | 0.020° |
| source | upper-right | 60.000° / 45.000° | 59.994° / 44.989° | 0.020° |
| source | right | 0.000° / 45.000° | 0.000° / 44.995° | 0.020° |
| source | lower-right | 300.000° / 45.000° | 300.006° / 44.989° | 0.020° |
| source | lower-left | 240.000° / 45.000° | 239.994° / 44.989° | 0.020° |
| source | left | 180.000° / 45.000° | 180.000° / 44.995° | 0.020° |
