//! Stage D — SRT/VTT output formatting.
//!
//! Port of the slice of `stable_whisper.text_output` submate's pipeline emits
//! at the end: [`to_srt_vtt`], the [`sec2srt`] / [`sec2vtt`] timestamp
//! formatters, and the word-level highlight / cue-timing tagging (the SRT
//! `<font>` path via `words2segments`, the VTT `<timestamp>`-marker path via
//! `to_vtt_word_level_segments`) with the inter-word gap filling upstream does.
//!
//! ## Parity scope
//!
//! The byte-for-byte falsifier is `to_srt_vtt(word_level=false)` against the
//! captured `03.srt` / `03.vtt` goldens (`parity::output`). Those goldens come
//! from a *real* end-to-end transcription run (see
//! `fixtures/capture/capture_stablets.py`), so their decoded text differs from
//! the `00_raw` / `02_suppress` JSON goldens (a separate, non-deterministic
//! Whisper decode) and they carry only segment-level timing+text — there is no
//! word-level `03` golden. The parity test therefore reconstructs the final
//! segments from `03.srt` and asserts both the SRT round-trip *and* the
//! cross-format VTT emission are byte-identical, which pins [`sec2srt`],
//! [`sec2vtt`], block assembly, and `finalize_text` against the real fixtures.
//! The word-level tagging / gap-filling paths (not present in any fixture) are
//! pinned by the unit tests in this module, transcribed from the upstream
//! `words2segments` / `to_vtt_word_level_segments` semantics.
//!
//! ## What is faithful to upstream
//!
//! * `sec2vtt`: `f"{hh:0>2.0f}:{mm:0>2.0f}:{ss:0>6.3f}"` over Python's
//!   `divmod`-derived `hh`/`mm`/`ss`; `sec2srt` is `sec2vtt` with `.` → `,`.
//! * `finalize_text` (strip path): `text.strip().replace('\n ', '\n')`.
//! * Segment blocks: `segment2srtblock` / `segment2vttblock`, joined with
//!   `"\n\n"`; VTT is prefixed with `"WEBVTT\n\n"`. No trailing newline (upstream
//!   returns the joined string; the file goldens have no trailing newline).
//! * Word-level SRT tag default `('<font color="#00ff00">', '</font>')`, with
//!   `words2segments` exploding each word into its own cue and inserting empty
//!   gap cues where `next_start - curr_end != 0`.
//! * Word-level VTT cue-timing markers via `to_vtt_word_level_segments`.

use std::borrow::Cow;

use crate::model::{Segment, WhisperResult};

/// SRT word-highlight tag default: `('<font color="#00ff00">', '</font>')`.
const SRT_TAG: (&str, &str) = ("<font color=\"#00ff00\">", "</font>");

/// `min_dur` default `result_to_srt_vtt` passes to `apply_min_dur`.
///
/// Kept for documentation parity; the falsifier exercises the post-pipeline
/// final result whose segments already exceed this, and the ported
/// [`to_srt_vtt`] takes its segments verbatim from the [`WhisperResult`] (the
/// `apply_min_dur` merge pass is its own stage, not part of D).
pub const DEFAULT_MIN_DUR: f64 = 0.02;

/// `sec2hhmmss`: split seconds into `(hh, mm, ss)` the way Python's nested
/// `divmod` does, keeping `ss` (and the integer-valued `hh`/`mm`) as floats so
/// the downstream format specifiers round identically.
fn sec2hhmmss(seconds: f64) -> (f64, f64, f64) {
    let mm = seconds.div_euclid(60.0);
    let ss = seconds.rem_euclid(60.0);
    let hh = mm.div_euclid(60.0);
    let mm = mm.rem_euclid(60.0);
    (hh, mm, ss)
}

/// `sec2vtt`: `f"{hh:0>2.0f}:{mm:0>2.0f}:{ss:0>6.3f}"`.
///
/// `{:02.0}` zero-pads the integer-rounded `hh`/`mm` to width 2 (wider if they
/// overflow), and `{:06.3}` formats `ss` to 3 decimals zero-padded to width 6
/// (e.g. `02.440`). Rust's `{:.3}` rounds half-to-even, matching Python's
/// `format`.
#[must_use]
pub fn sec2vtt(seconds: f64) -> String {
    let (hh, mm, ss) = sec2hhmmss(seconds);
    format!("{hh:02.0}:{mm:02.0}:{ss:06.3}")
}

/// `sec2srt`: `sec2vtt(seconds).replace(".", ",")`.
#[must_use]
pub fn sec2srt(seconds: f64) -> String {
    sec2vtt(seconds).replace('.', ",")
}

/// `finalize_text` with `strip=True` (the only mode submate uses):
/// `text.strip().replace('\n ', '\n')`.
fn finalize_text(text: &str) -> String {
    text.trim().replace("\n ", "\n")
}

/// One SRT block: `segment2srtblock(segment, idx)` —
/// `"{idx}\n{start} --> {end}\n{finalize_text(text)}"`.
fn segment2srtblock(start: f64, end: f64, text: &str, idx: usize) -> String {
    format!(
        "{idx}\n{} --> {}\n{}",
        sec2srt(start),
        sec2srt(end),
        finalize_text(text)
    )
}

/// One VTT block: `segment2vttblock(segment)` —
/// `"{start} --> {end}\n{finalize_text(text)}"`.
fn segment2vttblock(start: f64, end: f64, text: &str) -> String {
    format!("{} --> {}\n{}", sec2vtt(start), sec2vtt(end), finalize_text(text))
}

/// A flattened output cue: `start`/`end` timing plus the (already tagged) text.
///
/// Mirrors the `dict(text=..., start=..., end=...)` rows the upstream
/// `to_word_level_*` / `words2segments` helpers build before block assembly.
#[derive(Debug, Clone, PartialEq)]
pub struct OutCue {
    /// Cue text (may carry highlight tags / cue-timing markers).
    pub text: String,
    /// Cue start in seconds.
    pub start: f64,
    /// Cue end in seconds.
    pub end: f64,
}

/// `words2segments`: explode one segment's words into per-word SRT cues,
/// highlighting the word at index `i` with `tag` and inserting empty gap cues
/// where consecutive words are not contiguous.
///
/// Faithful to upstream, including:
/// * timings rounded to 3 decimals (`round(_, 3)`) before comparison/emit,
/// * a gap cue `dict(word='', start=curr_end, end=next_start)` inserted when
///   `next_start - curr_end != 0`,
/// * the highlighted word's leading space being pulled *outside* the tag
///   (`" {tag0}{word[1:]}{tag1}"`), and empty/space words never tagged.
fn words2segments(words: &[(String, f64, f64)], tag: (&str, &str)) -> Vec<OutCue> {
    // Build the gap-filled word list (text, start, end), rounding like upstream.
    let mut filled: Vec<(String, f64, f64)> = Vec::new();
    for (i, (word, start, end)) in words.iter().enumerate() {
        let curr_end = round3(*end);
        filled.push((word.clone(), round3(*start), curr_end));
        if i + 1 < words.len() {
            let next_start = round3(words[i + 1].1);
            if next_start - curr_end != 0.0 {
                filled.push((String::new(), curr_end, next_start));
            }
        }
    }

    // For each filled row, the cue text is the whole row sequence with only the
    // matching index tagged (upstream's `add_tag(i)`).
    let mut cues = Vec::with_capacity(filled.len());
    for i in 0..filled.len() {
        let text: String = filled
            .iter()
            .enumerate()
            .map(|(idx, (word, _, _))| {
                if !word.is_empty() && word != " " && idx == i {
                    if let Some(rest) = word.strip_prefix(' ') {
                        format!(" {}{rest}{}", tag.0, tag.1)
                    } else {
                        format!("{}{word}{}", tag.0, tag.1)
                    }
                } else {
                    word.clone()
                }
            })
            .collect();
        cues.push(OutCue { text, start: filled[i].1, end: filled[i].2 });
    }
    cues
}

/// `to_vtt_word_level_segments`'s `to_segment_string`: keep one cue per segment
/// but splice cue-timing markers `<HH:MM:SS.mmm>` between words.
fn vtt_word_level_segment(
    seg_start: f64,
    seg_end: f64,
    words: &[(String, f64, f64)],
) -> OutCue {
    let mut s = String::new();
    let mut prev_end = 0.0_f64;
    for (i, (word, start, end)) in words.iter().enumerate() {
        // Upstream mutates `word['word']` in place in the gap branch, then the
        // shared append below uses the (possibly stripped) word; track that
        // with a local override rather than mutating the input.
        let mut word_text: Cow<'_, str> = Cow::Borrowed(word.as_str());
        if i != 0 {
            let curr_start = *start;
            if prev_end == curr_start {
                s.push_str(&format!("<{}>", sec2vtt(curr_start)));
            } else {
                if s.ends_with(' ') {
                    s.pop();
                } else if let Some(rest) = word.strip_prefix(' ') {
                    word_text = Cow::Owned(rest.to_owned());
                }
                s.push_str(&format!("<{}> <{}>", sec2vtt(prev_end), sec2vtt(curr_start)));
            }
        }
        s.push_str(&word_text);
        prev_end = *end;
    }
    OutCue { text: s, start: seg_start, end: seg_end }
}

/// Round to 3 decimals half-to-even, matching Python's `round(x, 3)` for the
/// finite timings here.
fn round3(value: f64) -> f64 {
    let factor = 1000.0;
    (value * factor).round_ties_even() / factor
}

/// `result_to_srt_vtt` / `WhisperResult.to_srt_vtt`, restricted to the modes
/// submate uses (`segment_level=True`, default tags, `strip=True`,
/// `reverse_text=False`).
///
/// * `word_level=false`: one block per segment, segment-level timing+text.
/// * `word_level=true`: SRT uses per-word `<font>` highlight cues with gap
///   filling; VTT keeps one cue per segment with `<timestamp>` markers.
///
/// Returns the joined string (no trailing newline), exactly what
/// `result_to_any` returns when `filepath is None`.
#[must_use]
pub fn to_srt_vtt(result: &WhisperResult, word_level: bool, vtt: bool) -> String {
    if vtt {
        let cues = vtt_cues(&result.segments, word_level);
        let blocks: Vec<String> = cues
            .iter()
            .map(|c| segment2vttblock(c.start, c.end, &c.text))
            .collect();
        format!("WEBVTT\n\n{}", blocks.join("\n\n"))
    } else {
        let cues = srt_cues(&result.segments, word_level);
        cues.iter()
            .enumerate()
            .map(|(i, c)| segment2srtblock(c.start, c.end, &c.text, i + 1))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// `OutputFormat.JSON`: serialize the full result `to_dict()` to a compact
/// single-line JSON string, matching submate's `json.dumps(result.to_dict())`.
///
/// Value-parity (not byte-parity): `serde_json` emits canonical separators
/// (`,`/`:`) and may order keys differently from Python's `json.dumps`, but the
/// emitted string parses back to the same `Value` as the golden `to_dict()`.
#[must_use]
pub fn to_json(result: &WhisperResult) -> String {
    serde_json::to_string(&result.to_dict()).expect("to_dict() Value always serializes")
}

/// `OutputFormat.TXT`: the result's full transcript text (concatenated segment
/// text, no timestamps), matching submate's plain-text export.
#[must_use]
pub fn to_txt(result: &WhisperResult) -> String {
    result.text()
}

/// Build the SRT cue list: segment-level rows, or per-word highlight rows.
fn srt_cues(segments: &[Segment], word_level: bool) -> Vec<OutCue> {
    if word_level {
        segments
            .iter()
            .flat_map(|s| words2segments(&segment_words(s), SRT_TAG))
            .collect()
    } else {
        segments
            .iter()
            .map(|s| OutCue { text: s.text(), start: s.start(), end: s.end() })
            .collect()
    }
}

/// Build the VTT cue list: segment-level rows, or per-segment cue-timing rows.
fn vtt_cues(segments: &[Segment], word_level: bool) -> Vec<OutCue> {
    if word_level {
        segments
            .iter()
            .map(|s| vtt_word_level_segment(s.start(), s.end(), &segment_words(s)))
            .collect()
    } else {
        segments
            .iter()
            .map(|s| OutCue { text: s.text(), start: s.start(), end: s.end() })
            .collect()
    }
}

/// Extract a segment's `(word, start, end)` rows for the word-level paths.
fn segment_words(seg: &Segment) -> Vec<(String, f64, f64)> {
    seg.words
        .as_ref()
        .map(|ws| ws.iter().map(|w| (w.word.clone(), w.start(), w.end())).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sec2vtt_matches_python_format() {
        assert_eq!(sec2vtt(0.0), "00:00:00.000");
        assert_eq!(sec2vtt(2.44), "00:00:02.440");
        assert_eq!(sec2vtt(5.9), "00:00:05.900");
        assert_eq!(sec2vtt(10.36), "00:00:10.360");
        assert_eq!(sec2vtt(3661.5), "01:01:01.500");
    }

    #[test]
    fn sec2srt_swaps_decimal_for_comma() {
        assert_eq!(sec2srt(2.44), "00:00:02,440");
        assert_eq!(sec2srt(10.36), "00:00:10,360");
    }

    #[test]
    fn finalize_text_strips_and_dedents_wrapped_lines() {
        assert_eq!(finalize_text("  hello world  "), "hello world");
        // `\n ` (newline + space) collapses to `\n`.
        assert_eq!(finalize_text("a\n b"), "a\nb");
    }

    #[test]
    fn words2segments_tags_and_fills_gaps() {
        // Contiguous words: no gap cue, two cues, each highlighting its word.
        let words = vec![
            (" Hello".to_string(), 0.0, 0.5),
            (" world".to_string(), 0.5, 1.0),
        ];
        let cues = words2segments(&words, SRT_TAG);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].text, " <font color=\"#00ff00\">Hello</font> world");
        assert_eq!(cues[1].text, " Hello <font color=\"#00ff00\">world</font>");
    }

    #[test]
    fn words2segments_inserts_empty_gap_cue() {
        // 0.5 -> 0.8 gap: an empty cue is inserted between the two words.
        let words = vec![
            (" Hi".to_string(), 0.0, 0.5),
            (" there".to_string(), 0.8, 1.2),
        ];
        let cues = words2segments(&words, SRT_TAG);
        assert_eq!(cues.len(), 3);
        // Middle cue is the gap: empty word, spanning 0.5..0.8, no tag applied.
        assert_eq!(cues[1].start, 0.5);
        assert_eq!(cues[1].end, 0.8);
        assert_eq!(cues[1].text, " Hi there");
    }

    #[test]
    fn vtt_word_level_inserts_cue_timing_markers() {
        // Contiguous: a single `<curr_start>` marker between words.
        let words = vec![
            (" Hello".to_string(), 0.0, 0.5),
            (" world".to_string(), 0.5, 1.0),
        ];
        let cue = vtt_word_level_segment(0.0, 1.0, &words);
        assert_eq!(cue.text, " Hello<00:00:00.500> world");
    }
}
