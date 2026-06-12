# Port SRT/VTT parsing + round-trip

**META note (round 3 unblock):** previously parked in `needs-human/` as if it
needed a human/credential. It does not — this is a **pure-data capture**
(`capture_subtitle.py`, mirroring `capture_paths.py`). `srt`, `pysubs2`, and the
`submate` package all import in the available nix python env, so META capture
pre-pass (or an implementer) can author and run it. The original reroot
(1783cdd) was a *porter touching denylisted fixtures*, not a real capture gate.
Returned to `backlog/` per the meta-contention pure-data pre-pass rule.

**round-trip source of truth:** `submate/translation.py` uses
`srt.parse(content)` then `srt.compose(subs)` (see `_translate_srt_content`,
lines ~357/363) — that is the byte-parity target, NOT `submate/subtitle.py`
(which is subtitle *discovery*/language detection only).

**blocked-by:** capture: emit Python `srt`/`pysubs2` round-trip goldens into
`rust/fixtures/subtitle/` via a dedicated capture commit (see precondition below).

## what
Hand-roll SRT and VTT cue parsing + serialization matching the Python `srt` + `pysubs2` output byte-for-byte (needed so translation re-emits identical files).

## where
`rust/crates/submate-subtitle/src/lib.rs`.

## why
Translation parses and re-emits these; byte-parity avoids spurious diffs.

## capture precondition (do NOT skip — denylist-protected fixtures)
The goldens `rust/fixtures/subtitle/<name>.{srt,vtt}` and their round-trip outputs
do NOT exist yet and CANNOT be authored by the Rust port — that makes parity
self-referential. `rust/fixtures/` is `mergeDenylist`-protected, which is why a
prior attempt was rerouted (denylist hit, 1783cdd). Add
`rust/fixtures/capture/capture_subtitle.py` (mirror `capture_paths.py`) that
parses representative `.srt`/`.vtt` inputs through Python `srt`/`pysubs2` and dumps
the re-serialized bytes as goldens, list them in `rust/fixtures/README.md`, and
land the fixtures via a deliberate capture commit — NOT as a side effect of
porting code. Only then port the parser against them.

## falsifies
`cargo test -p submate-subtitle parity::srt_roundtrip` re-emits each
`rust/fixtures/subtitle/*.srt` byte-identically against the Python-captured golden;
likewise `parity::vtt_roundtrip` for `.vtt`.
