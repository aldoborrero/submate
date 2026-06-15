//! Structural parity for the post-decode assembly pipeline.
//!
//! The `transcribe` falsifier drives the post-decode assembly stages
//! ([`submate_whisper::assemble_result`]) from the *captured* raw transcription
//! (`stablets/clipA/00_raw.json`, a real decode) and the same `audio.f32` the
//! silence stage reads, then compares the finished segments to the golden
//! `transcribe/clipA.segments.json` within [`SegTol`] (count ±1, time ±200 ms,
//! text-ratio ≥ 0.9) — structural, not exact, because the golden was produced
//! by a different Whisper engine.
//!
//! The captured raw and the golden come from *separate* non-deterministic
//! decodes of the synthetic clip (decoded three independent times). They agree on
//! the first two of three lines but diverge by one word in the last
//! (own two ⇄ old into), so the comparison is split: count + per-segment timing
//! are checked against the whole golden (the regroup re-splits the 2 raw
//! segments into the golden's 3 lines at the same boundaries), and the
//! text-ratio is checked on the segments the two decodes agree on.
//!
//! The exact per-stage math is pinned byte-for-byte by the `stable-ts` crate's
//! own parity tests; this test guards their composition.

use fixtures::{Seg, SegTol, assert_segments_close, golden, load_f32, segs_from_json};
use submate_whisper::{
    AssembleOptions, WhisperResult, WhisperSegment, WhisperWord, assemble_result,
};

/// Parse the captured raw transcription (`00_raw.json`, a stable-ts `to_dict()`
/// dump) into the inference-shaped [`WhisperResult`] this crate produces, so
/// `assemble_result` consumes exactly what real whisper.cpp inference would.
fn raw_from_golden(rel: &str) -> WhisperResult {
    let v = golden(rel);
    let language = v
        .get("language")
        .and_then(|l| l.as_str())
        .unwrap_or("en")
        .to_string();
    let segments = v
        .get("segments")
        .and_then(|s| s.as_array())
        .expect("00_raw has a segments array")
        .iter()
        .map(|seg| {
            let words = seg
                .get("words")
                .and_then(|w| w.as_array())
                .map(|ws| {
                    ws.iter()
                        .map(|w| WhisperWord {
                            word: w["word"].as_str().unwrap_or_default().to_string(),
                            start: w["start"].as_f64().unwrap_or(0.0),
                            end: w["end"].as_f64().unwrap_or(0.0),
                            probability: w["probability"].as_f64().unwrap_or(0.0),
                        })
                        .collect()
                })
                .unwrap_or_default();
            WhisperSegment {
                text: seg["text"].as_str().unwrap_or_default().to_string(),
                start: seg["start"].as_f64().unwrap_or(0.0),
                end: seg["end"].as_f64().unwrap_or(0.0),
                words,
            }
        })
        .collect();
    WhisperResult {
        language,
        text: v["text"].as_str().unwrap_or_default().trim().to_string(),
        segments,
    }
}

/// Tests live in a module named `parity` so the falsifier path is
/// `parity::transcribe` (matching `cargo test -p submate-whisper parity::transcribe`).
mod parity {
    use super::*;

    /// Structural pipeline falsifier (no model needed): the post-decode stages turn
    /// the captured raw transcription into the golden's segment *structure* — same
    /// count, same per-segment timing, same text for the segments the two
    /// independent decodes agree on.
    #[test]
    fn transcribe() {
        let raw = raw_from_golden("stablets/clipA/00_raw.json");
        let audio = load_f32("stablets/clipA/audio.f32");

        let transcription = assemble_result(&raw, &AssembleOptions::default(), &audio)
            .expect("pipeline stages apply with the default assemble options");

        let actual: Vec<Seg> = transcription
            .segments()
            .into_iter()
            .map(|s| Seg {
                start: s.start,
                end: s.end,
                text: s.text,
            })
            .collect();
        let golden_segs = segs_from_json(&golden("transcribe/clipA.segments.json"));

        // Count + per-segment timing match the golden exactly (regroup re-splits the
        // 2 raw segments into the golden's 3 lines at the same boundaries).
        assert_eq!(
            actual.len(),
            golden_segs.len(),
            "regroup+suppress must yield the golden's segment count"
        );
        let tol = SegTol::default();
        for (i, (a, g)) in actual.iter().zip(&golden_segs).enumerate() {
            let ds = ((a.start - g.start).abs() * 1000.0) as u64;
            let de = ((a.end - g.end).abs() * 1000.0) as u64;
            assert!(
                ds <= tol.time_ms && de <= tol.time_ms,
                "segment {i} timing drift start={ds}ms end={de}ms > {}ms",
                tol.time_ms
            );
        }

        // The two leading segments are identical across the two decodes; assert
        // their text structurally. The trailing segment differs by one cross-run
        // word swap (own two ⇄ old into) — that full-text check is the job of the
        // model-gated `transcribe_real_decode` falsifier against a single decode.
        assert_segments_close(&actual[..2], &golden_segs[..2], tol);
    }
}
