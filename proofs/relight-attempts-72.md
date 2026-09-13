# Six-facing relight attempt

`relight-rest-72.png` shows two rest facings from the bounded painter-relight attempt at shipped scale. `relight-swing-72.gif` shows the 60° turn between neighbouring attempted relights at shipped scale. The animation has no discrete topology jump; consecutive coalesced frames have 3.22% maximum normalized pixel RMSE, including the intended rotation and crossfade.

The unchanged limit is 25°. Nine sets passed the sphere-shape check but failed the requested-direction check:

| Texture | Maximum direction error |
| --- | ---: |
| arm | 33.3° |
| bonder | 36.1° |
| converter-amber | 33.2° |
| converter-plum | 43.6° |
| output-2 | 43.2° |
| second-bond | 45.3° |
| source | 35.9° |
| atom-amber | 32.3° |
| bond-double | 45.6° |

The remaining six sets failed the sphere-shape check first: output-1 at 27.8°, output-3 at 31.0°, reification at 25.1°, atom-base at 29.8°, atom-plum at 27.8°, and bond-single at 25.8°.
