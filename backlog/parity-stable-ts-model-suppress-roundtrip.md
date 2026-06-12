# Parity: stable-ts WhisperResult roundtrip over populated nonspeech_sections

## what
Extend `stable-ts`'s model parity coverage to the WhisperResult-shaped goldens
that the current single test never touches. `rust/crates/stable-ts/tests/parity.rs`
has exactly one test, `model_roundtrip`, and it parses + re-emits ONLY
`rust/fixtures/stablets/clipA/00_raw.json`. That fixture has
`nonspeech_sections: []` and `regroup_history: ""` (both empty). Four sibling
goldens of the **same WhisperResult `to_dict()` shape** are committed and
**unconsumed by any test**:

- `rust/fixtures/stablets/clipA/01_regroup_0_clamp_max.json`
- `rust/fixtures/stablets/clipA/01_regroup_1_split_by_length.json`
- `rust/fixtures/stablets/clipA/01_regroup_2_split_by_length.json`
- `rust/fixtures/stablets/clipA/02_suppress.json`  ← the load-bearing one:
  `nonspeech_sections` here holds **27** `{"start": f, "end": f}` dicts, and
  every segment carries post-DSP word timings.

So `WhisperResult::{from_value,to_dict}` handling of a **non-empty**
`nonspeech_sections` and of segments whose word `start`/`end` were rewritten by
suppress is currently asserted by zero parity test. A regression that dropped,
reordered, or re-typed `nonspeech_sections` (it is kept verbatim as a
`serde_json::Value`), or that mis-derived `text`/`segments` for the populated
case, would go undetected.

## where
`rust/crates/stable-ts/tests/parity.rs`, alongside the existing
`model_roundtrip`. Add a `suppress_roundtrip` test (and optionally a
table-driven loop over the three `01_regroup_*` stages). Each does the same
three lines the existing test does:

```rust
let raw = golden("stablets/clipA/02_suppress.json");
let actual = WhisperResult::from_value(&raw).to_dict();
assert_json_eq(&actual, &raw);
```

No production code change is required — this is a pure coverage gap. Verified:
all five `0[0-2]*.json` goldens already roundtrip green under the current
`model.rs` (`from_value`/`to_dict` keep `ori_dict` and `nonspeech_sections`
verbatim, and the suppress golden satisfies `text == concat(segment.text)` so
the derived-`text` re-emit matches). The fixtures are unchanged; only the test
file gains assertions.

## why
The stable-ts slice is the repo's highest-risk port and is explicitly tested
"stage by stage against exact intermediate data" (see `rust/fixtures/README.md`,
"The stable-ts staged goldens"). The capture emits `02_suppress.json`
specifically so the populated-`nonspeech_sections` form is pinned; leaving it
untested defeats the point of capturing it. `02_suppress` is also the only
model-shaped golden where word timings are non-trivial (post-`suppress_silence`),
so it is the strongest available regression guard for the data model before the
B/C/D stages (regroup/suppress/output) are ported on top of it.

## falsifies
`cargo test -p stable-ts --test parity suppress_roundtrip` exists and passes:
parsing `rust/fixtures/stablets/clipA/02_suppress.json` into `WhisperResult`
and re-emitting via `to_dict()` yields a `serde_json::Value` structurally equal
(float-aware, via `parity::assert_json_eq`) to the golden — including the
27-element `nonspeech_sections` array and all post-DSP word timings. Extending
the same assertion to the three `01_regroup_*` goldens passes identically.
