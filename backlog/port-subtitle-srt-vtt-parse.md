# Port SRT/VTT parsing + round-trip

**META capture pre-pass LANDED (round 3 cleanup):** `capture_subtitle.py` is
authored and run; goldens `rust/fixtures/subtitle/{basic.srt,single.srt,basic.vtt}`
are committed (the parse->compose / from_string->to_string round-trips). The
oracle now exists — the porter diffs against it and must NOT touch
`rust/fixtures/**`. Item returned to `backlog/` for automated pickup.

**META note (r2 round 1 confirm):** env re-verified — `python3 -c "import srt,
pysubs2"` succeeds in the active nix devshell (`srt` 1.8.0, `pysubs2` present),
so the capture is runnable **now** with no external runtime. The stale
`backlog/tried/` abandon record for this item was removed this round (it
contradicted the unpark at `a1d95d3`). Next round's META/capture pre-pass should
**execute** `capture_subtitle.py` and land the goldens in a deliberate capture
commit before dispatch — do NOT re-park to `needs-human/`; there is no gate.

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
