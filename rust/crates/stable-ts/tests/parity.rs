//! Parity tests against golden fixtures captured from `stable_whisper.result`.
//!
//! Each test parses a captured `to_dict()` JSON golden into the ported
//! [`WhisperResult`] and re-emits it via [`WhisperResult::to_dict`]; the result
//! must equal the golden JSON value exactly (`parity::assert_json_eq` does a
//! structural, float-aware comparison — the capture writes sorted-key/pretty
//! JSON, but the Rust side compares parsed `serde_json::Value`s, so formatting
//! is irrelevant). The goldens span the empty (`00_raw`), regroup-staged
//! (`01_regroup_*`), and populated-`nonspeech_sections` (`02_suppress`) shapes.

use parity::{assert_f32_close, assert_json_eq, assert_str_eq, fixture_path, golden, load_f32};
use stable_ts::{
    apply_regroup_op, audio2timings, ops_to_value, parse_regroup_algo, set_current_as_orig,
    suppress_silence, update_nonspeech_sections, WhisperResult, DEFAULT_MIN_WORD_DUR,
};

/// The submate config regroup string (see `fixtures/capture/capture_stablets.py`).
const REGROUP: &str = "cm_sl=84_sl=42++++++1";

#[test]
fn model_roundtrip() {
    let raw = golden("stablets/clipA/00_raw.json");
    let result = WhisperResult::from_value(&raw);
    let actual = result.to_dict();
    assert_json_eq(&actual, &raw);
}

/// Same roundtrip as `model_roundtrip`, but over `02_suppress.json` — the only
/// model-shaped golden whose `nonspeech_sections` is non-empty (27
/// `{"start", "end"}` dicts) and whose segments carry post-`suppress_silence`
/// word timings. Guards against a regression that drops, reorders, or re-types
/// the verbatim `nonspeech_sections` value or mis-derives `text`/`segments` for
/// the populated case.
#[test]
fn suppress_roundtrip() {
    let raw = golden("stablets/clipA/02_suppress.json");
    let actual = WhisperResult::from_value(&raw).to_dict();
    assert_json_eq(&actual, &raw);
}

/// `audio2loudness(audio.f32)` must match the Python-captured `loudness.f32`
/// (dumped straight from `stable_whisper.stabilization.nonvad.audio2loudness`)
/// within `1e-6`. This pins the abs -> 0.1%-topk threshold -> normalize ->
/// `f32` linear-interpolate-to-token-count chain against torch's exact output.
#[test]
fn audio2loudness() {
    let audio = load_f32("stablets/clipA/audio.f32");
    let golden = load_f32("stablets/clipA/loudness.f32");
    let actual = stable_ts::audio2loudness(&audio).expect("clipA is long enough for loudness");
    assert_f32_close(&actual, &golden, 1e-6);
}

/// `wav2mask(audio.f32)` must match the Python-captured `mask.f32` (the bool
/// mask `nonvad.wav2mask` returns, dumped as `0.0`/`1.0`) within `1e-6`. This
/// pins the avg-pool (`k=5`, reflect) -> quantize (`q_levels=20`) -> timing
/// roundtrip -> invert chain.
#[test]
fn wav2mask() {
    let audio = load_f32("stablets/clipA/audio.f32");
    let golden = load_f32("stablets/clipA/mask.f32");
    let mask = stable_ts::wav2mask(&audio).expect("clipA has both silence and audible audio");
    let actual: Vec<f32> = mask.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect();
    assert_f32_close(&actual, &golden, 1e-6);
}

/// `parse_regroup_algo` produces (`stablets/regroup_parse.json`).
#[test]
fn regroup_parse() {
    let golden_ops = golden("stablets/regroup_parse.json");
    let ops = parse_regroup_algo("cm_sl=84_sl=42++++++1").expect("known methods");
    assert_json_eq(&ops_to_value(&ops), &golden_ops);
}

/// B2 apply falsifier: each staged regroup op, applied *in isolation* to a fresh
/// `WhisperResult` parsed from `00_raw.json`, must reproduce its golden exactly.
///
/// This mirrors `capture_stablets.py`, which rebuilds a fresh result from
/// `raw_dict` per op (so stages don't compound) and dumps `to_dict()`. The op
/// list comes from the same `REGROUP` string `parse_regroup_algo` parses, and
/// the golden filename per op is `01_regroup_<i>_<method>.json`.
#[test]
fn regroup_apply() {
    let raw = golden("stablets/clipA/00_raw.json");
    let ops = parse_regroup_algo(REGROUP).expect("known methods");

    for (i, op) in ops.iter().enumerate() {
        let golden_name = format!("stablets/clipA/01_regroup_{i}_{}.json", op.method);
        let expected = golden(&golden_name);

        let mut result = WhisperResult::from_value(&raw);
        apply_regroup_op(&mut result, op).expect("staged op is applicable in B2");

        assert_json_eq(&result.to_dict(), &expected);
    }
}

/// B2 apply falsifier for the split-by-duration (`sd`) and merge-all-segments
/// (`ms`) ops, which parse (B1) but were unrunnable until this port.
///
/// Each op string is parsed by the same `parse_regroup_algo` the rest of the
/// pipeline uses and applied in isolation to a fresh `WhisperResult` rebuilt
/// from `00_raw.json`, exactly as `capture_stablets.py` produced the golden
/// (`fresh.split_by_duration(max_dur=4)` / `fresh.merge_all_segments()`). The
/// re-emitted `to_dict()` must match the captured golden value.
#[test]
fn regroup_apply_duration_merge() {
    let raw = golden("stablets/clipA/00_raw.json");

    for (algo, golden_name) in [
        ("sd=4", "stablets/clipA/01c_regroup_sd.json"),
        ("ms", "stablets/clipA/01c_regroup_ms.json"),
    ] {
        let expected = golden(golden_name);
        let ops = parse_regroup_algo(algo).expect("known methods");

        let mut result = WhisperResult::from_value(&raw);
        for op in &ops {
            apply_regroup_op(&mut result, op).expect("sd/ms are applicable in B2");
        }

        assert_json_eq(&result.to_dict(), &expected);
    }
}

/// C2 apply falsifier: the full non-VAD suppress-silence stage.
///
/// `capture_stablets.py` produces `02_suppress.json` by re-running the engine
/// with `regroup=False, suppress_silence=True` — i.e. it applies suppression to
/// the *unregrouped* result (`00_raw`), not to a regroup stage. So this rebuilds
/// `00_raw`, derives the silence ranges from `audio.f32` via
/// [`audio2timings`] (= `mask2timing(wav2mask(..))`), applies the per-word
/// [`suppress_silence`] and [`update_nonspeech_sections`] with the same defaults
/// `transcribe_stable` uses (`min_word_dur=0.1`, `nonspeech_error=0.1`,
/// `word_level=True`, `use_word_position=True`), and checks the result against
/// the golden — pinning both the clipped word timings and the populated
/// `nonspeech_sections`.
#[test]
fn suppress() {
    let raw = golden("stablets/clipA/00_raw.json");
    let expected = golden("stablets/clipA/02_suppress.json");
    let audio = load_f32("stablets/clipA/audio.f32");

    let (starts, ends) = audio2timings(&audio).expect("clipA has suppressible silence");

    let mut result = WhisperResult::from_value(&raw);
    suppress_silence(&mut result, &starts, &ends, DEFAULT_MIN_WORD_DUR, 0.1);
    update_nonspeech_sections(&mut result, &starts, &ends);
    // transcribe_stable snapshots the suppressed state into `ori_dict` right
    // after the stage, so the golden's `ori_dict` is itself suppressed.
    set_current_as_orig(&mut result);

    assert_json_eq(&result.to_dict(), &expected);
}

/// D output falsifier: `to_srt_vtt(word_level=false)` must reproduce the real
/// `03.srt` / `03.vtt` goldens byte-for-byte.
///
/// Those goldens are dumped from a *separate* end-to-end transcription run
/// (`capture_stablets.py`'s `final = model.transcribe_stable(regroup=REGROUP,
/// suppress_silence=True)`), whose non-deterministic Whisper decode produced
/// different text than the `00_raw`/`02_suppress` JSON goldens — so the final
/// result is not byte-reproducible from any captured JSON, and (being
/// `word_level=False`) the goldens carry only segment-level timing+text.
///
/// We therefore reconstruct the final segments from `03.srt` (its blocks are
/// exactly `{start, end, text}`), build a [`WhisperResult`] from them, and
/// assert that emitting SRT reproduces `03.srt` (round-trip) and that emitting
/// VTT from the *same* segments reproduces `03.vtt` (cross-format).
///
/// The VTT direction is non-circular — those segments are parsed from the SRT,
/// never the VTT — so it pins `sec2vtt`, `WEBVTT` framing, and block assembly
/// independently. Together they nail `sec2srt`/`sec2vtt`, `finalize_text`, and
/// the `\n\n`-joined block layout against the real fixtures. The word-level
/// `<font>`/`<timestamp>` paths (absent from every fixture) are pinned by the
/// unit tests in `stable_ts::output`.
#[test]
fn output() {
    let srt_golden = std::fs::read_to_string(fixture_path("stablets/clipA/03.srt"))
        .expect("03.srt fixture present");
    let vtt_golden = std::fs::read_to_string(fixture_path("stablets/clipA/03.vtt"))
        .expect("03.vtt fixture present");

    // Reconstruct the final segments from the SRT blocks: `idx\nHH:MM:SS,mmm -->
    // HH:MM:SS,mmm\ntext...`. These are segment-level (`word_level=False`), so
    // no words are needed to re-emit.
    let segments: Vec<serde_json::Value> = srt_golden
        .split("\n\n")
        .map(|block| {
            let mut lines = block.lines();
            lines.next().expect("index line"); // the 1-based block index
            let ts = lines.next().expect("timestamp line");
            let (start_s, end_s) = ts.split_once(" --> ").expect("`start --> end`");
            let text = lines.collect::<Vec<_>>().join("\n");
            serde_json::json!({
                "start": parse_srt_ts(start_s),
                "end": parse_srt_ts(end_s),
                "text": text,
            })
        })
        .collect();

    let input = serde_json::json!({ "segments": segments });
    let result = WhisperResult::from_value(&input);

    assert_str_eq(
        &stable_ts::output::to_srt_vtt(&result, false, false),
        &srt_golden,
    );
    assert_str_eq(
        &stable_ts::output::to_srt_vtt(&result, false, true),
        &vtt_golden,
    );
}

/// Segment-level ASS output must be byte-identical to `output.ass`, which was
/// captured via `WhisperResult.to_ass(segment_level=True, word_level=False)` on
/// the `00_raw` result. Unlike the SRT/VTT goldens, this fixture comes from the
/// same `00_raw.json` we parse, so we feed `00_raw` directly. This pins the
/// header, the `Default` style line, `sec2ass`, and the `Dialogue` event layout
/// against the real stable_whisper output.
#[test]
fn output_ass() {
    let raw = golden("stablets/clipA/00_raw.json");
    let result = WhisperResult::from_value(&raw);
    let ass_golden = std::fs::read_to_string(fixture_path("stablets/clipA/output.ass"))
        .expect("output.ass fixture present");

    assert_str_eq(&stable_ts::output::to_ass(&result, false), &ass_golden);
}

/// JSON output falsifier (value-parity): `to_json` of the result parsed from
/// `00_raw.json` must parse back to the golden `output.json` Value (== the
/// result's `to_dict()`). Asserts the round-trip, not the byte layout —
/// `serde_json` separators / key order are formatting a JSON consumer ignores.
#[test]
fn output_json() {
    let raw = golden("stablets/clipA/00_raw.json");
    let expected = golden("stablets/clipA/output.json");
    let result = WhisperResult::from_value(&raw);

    let emitted = stable_ts::output::to_json(&result);
    let parsed: serde_json::Value =
        serde_json::from_str(&emitted).expect("to_json emits valid JSON");
    assert_json_eq(&parsed, &expected);
}

/// TXT output falsifier: `to_txt` of the result parsed from `00_raw.json` must
/// equal that golden's `text` field verbatim (the concatenated transcript).
#[test]
fn output_txt() {
    let raw = golden("stablets/clipA/00_raw.json");
    let expected = raw["text"].as_str().expect("00_raw.json has a string `text`");
    let result = WhisperResult::from_value(&raw);

    assert_str_eq(&stable_ts::output::to_txt(&result), expected);
}

/// Parse an `HH:MM:SS,mmm` SRT timestamp into seconds.
fn parse_srt_ts(ts: &str) -> f64 {
    let (hms, ms) = ts.split_once(',').expect("`HH:MM:SS,mmm`");
    let mut parts = hms.split(':');
    let hh: f64 = parts.next().unwrap().parse().unwrap();
    let mm: f64 = parts.next().unwrap().parse().unwrap();
    let ss: f64 = parts.next().unwrap().parse().unwrap();
    let mmm: f64 = ms.parse().unwrap();
    hh * 3600.0 + mm * 60.0 + ss + mmm / 1000.0
}
