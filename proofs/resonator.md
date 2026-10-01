# Paired resonance

[The firing clip](resonator.gif) shows one amber atom waiting in its seat, a second amber being carried into the other seat, and both turning into plum together without moving or disappearing. [The machine sheet](resonator-sheet.png) shows the empty housing, paired amber inputs and paired plum outputs, from left to right. Both are unscaled crops of native application captures at the shipped micro view.

The resonator requires two lone amber atoms. Missing, wrong-kind or bonded inputs refuse the entire reaction. Both atom identities and positions survive; no compound is consumed and no output cell is created. Its amber-and-base manufacturing recipe bootstraps before plum or cobalt. The reachability suite constructs the prerequisites and proves every dependent recipe.

Capture commands, from the repository root inside `nix-shell`:

```sh
mkdir -p scratch/resonator-frames
cargo run -- --shot scratch/resonator-sheet-full.png resonator-sheet 0
magick scratch/resonator-sheet-full.png -crop 600x160+410+220 +repage proofs/resonator-sheet.png
cargo run -- --shot scratch/resonator-frames resonator 0 10 400 1
FRAMES=scratch/resonator-frames art/gif.sh proofs/resonator.gif resonator 10 330:170:620:215
```
