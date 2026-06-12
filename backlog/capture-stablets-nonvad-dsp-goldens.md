# capture: stable-ts non-VAD DSP goldens (loudness.f32 + mask.f32)

**type:** deliberate fixture capture (NOT a port). Lands Python-provenance
goldens so the suppress-dsp port can be falsified non-vacuously.

## what
Extend `rust/fixtures/capture/capture_stablets.py` to also dump, for each
captured clip, the Python `stable_whisper.stabilization.nonvad` intermediate
outputs that the non-VAD silence DSP produces:

- `rust/fixtures/stablets/<clip>/loudness.f32` — output of `audio2loudness`
  applied to the already-captured `audio.f32` (abs → 0.1% threshold →
  normalize → linear-interp to token count).
- `rust/fixtures/stablets/<clip>/mask.f32` — output of `wav2mask`
  (avg-pool k=5, quantize q_levels=20, invert).

Reuse the existing `_dump_f32` helper. Import the real functions from
`stable_whisper.stabilization.nonvad` (e.g. `audio2loudness`, `wav2mask`);
if the public names differ, locate them in the installed `stable_whisper`
package and call the exact functions the non-VAD path uses — do NOT
reimplement the DSP in the capture script.

## why this is its own item
`port-stablets-suppress-dsp-C1` cycled ≥2 rounds (rerouted to needs-human,
reverted in 7d3abd2 for Rust-authored goldens with no Python provenance,
re-scoped). The gating work is the capture, and `rust/fixtures/` is
`mergeDenylist`-protected so it must land via a dedicated capture commit,
not as a side effect of porting. Splitting it lets a worker land just the
goldens, unblocking the DSP port cleanly.

## where
`rust/fixtures/capture/capture_stablets.py` (extend `main()` after the
`audio.f32` dump). Add the two new files to `rust/fixtures/README.md`.

## how to run (devshell, with the same clip used for audio.f32)
    python rust/fixtures/capture/capture_stablets.py /path/to/clip.wav --clip-name clipA

## done when
- `rust/fixtures/stablets/clipA/loudness.f32` and `mask.f32` exist with
  Python provenance, documented in `rust/fixtures/README.md`.
- Committed as a deliberate capture (message names the Python functions used).
- Unblocks `backlog/tried/port-stablets-suppress-dsp-C1.md` — move that item
  back to `backlog/` once the goldens land.
