# stable-ts (C1): suppress_silence DSP (audio2loudness + wav2mask)

**blocked-by:** capture: emit `loudness.f32`/`mask.f32` goldens from Python nonvad (see below), then port-stablets-model-A

## what
Port the non-VAD silence DSP: `audio2loudness` (abs → 0.1% threshold → normalize → linear interpolate to token count) and `wav2mask` (avg-pool k=5, quantize q_levels=20, invert). No ML model (vad=False default).

## where
`rust/crates/stable-ts/src/suppress_silence.rs`.

## why
Signal-processing core of timestamp stabilization; deterministic, so exact-within-1e-6 testable.

## capture precondition (do NOT skip — prior attempt reverted for this)
The goldens `rust/fixtures/stablets/<clip>/loudness.f32` and `mask.f32` do NOT
exist yet and CANNOT be authored by the Rust port — that makes parity
self-referential. `rust/fixtures/` is `mergeDenylist`-protected. Extend
`rust/fixtures/capture/capture_stablets.py` to also dump the Python
`stable_whisper.stabilization.nonvad` `audio2loudness`/`wav2mask` outputs for the
captured `audio.f32`, run it as a deliberate capture, add the two files to
`rust/fixtures/README.md`, and land the goldens via a dedicated capture commit —
NOT as a side effect of porting code. Only then port the DSP against them.

A prior merge (reverted in 7d3abd2) landed Rust-authored goldens that had no
Python provenance; the falsifier passed vacuously. The capture step is the work.

## falsifies
`cargo test -p stable-ts parity::wav2mask` feeds `rust/fixtures/stablets/*/audio.f32`
and matches the **Python-captured** mask golden within `1e-6` (via
`parity::assert_f32_close`); likewise `parity::audio2loudness` against `loudness.f32`.
