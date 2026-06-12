# stable-ts (C1): suppress_silence DSP (audio2loudness + wav2mask)

**blocked-by:** port-stablets-model-A

## what
Port the non-VAD silence DSP: `audio2loudness` (abs → 0.1% threshold → normalize → linear interpolate to token count) and `wav2mask` (avg-pool k=5, quantize q_levels=20, invert). No ML model (vad=False default).

## where
`rust/crates/stable-ts/src/suppress_silence.rs`.

## why
Signal-processing core of timestamp stabilization; deterministic, so exact-within-1e-6 testable.

## falsifies
`cargo test -p stable-ts parity::wav2mask` feeds `rust/fixtures/stablets/*/audio.f32` and matches the mask golden within `1e-6` (via `parity::assert_f32_close`).
