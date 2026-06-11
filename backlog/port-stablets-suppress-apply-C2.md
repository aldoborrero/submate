# stable-ts (C2): mask2timing + per-word suppress_silence

**blocked-by:** port-stablets-suppress-dsp-C1, port-stablets-regroup-splits-B2

## what
Port `mask2timing` (bool-mask transitions → silence ranges) and the per-word `suppress_silence` timestamp adjustment (overlap clip, min_word_dur=0.1, nonspeech tolerance).

## where
`rust/crates/stable-ts/src/suppress_silence.rs`.

## why
Applies the silence map to word timings — the user-visible timing correction.

## falsifies
`cargo test -p stable-ts parity::suppress` transforms `01_regroup_*.json` + `audio.f32` → `rust/fixtures/stablets/*/02_suppress.json` within `1e-6` timing tolerance.
