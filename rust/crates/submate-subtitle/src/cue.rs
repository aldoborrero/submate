//! Hand-rolled SRT and VTT cue parsing + serialization with byte-parity to the
//! Python round-trip used by `submate/translation.py`.
//!
//! Translation parses a subtitle file and re-emits it (`srt.parse` ->
//! `srt.compose` for SRT, pysubs2 `from_string` -> `to_string` for VTT), so the
//! Rust port must reproduce the *re-serialized* bytes, not merely a valid file.
//! This module mirrors the exact serialization rules of those two libraries.
//!
//! Times are kept in whole milliseconds, which is the resolution both formats
//! use and matches the captured goldens in `rust/fixtures/subtitle/`.

/// A single subtitle cue: a time span plus its (possibly multi-line) text.
///
/// `index` is the 1-based number from the source SRT block when present. VTT
/// cues carry no index of their own (pysubs2 numbers them on output), so it is
/// left as `None` for VTT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    pub index: Option<u64>,
    /// Start time, milliseconds.
    pub start_ms: i64,
    /// End time, milliseconds.
    pub end_ms: i64,
    /// Cue text. Line breaks are stored as `\n`.
    pub text: String,
}

// ---------------------------------------------------------------------------
// Timestamp formatting
// ---------------------------------------------------------------------------

/// Split milliseconds into `(hours, minutes, seconds, millis)`, normalized so
/// that `millis < 1000`, `seconds < 60`, and `minutes < 60`.
///
/// Mirrors `pysubs2.time.ms_to_times`. `srt.timedelta_to_srt_timestamp`
/// computes the same fields from a `timedelta`.
fn ms_to_parts(ms: i64) -> (i64, i64, i64, i64) {
    let ms = ms.max(0);
    let (h, rem) = (ms / 3_600_000, ms % 3_600_000);
    let (m, rem) = (rem / 60_000, rem % 60_000);
    let (s, ms) = (rem / 1_000, rem % 1_000);
    (h, m, s, ms)
}

/// Format milliseconds as an SRT timestamp `HH:MM:SS,mmm`.
///
/// Mirrors `srt.timedelta_to_srt_timestamp`.
fn format_srt_timestamp(ms: i64) -> String {
    let (h, m, s, ms) = ms_to_parts(ms);
    format!("{h:02}:{m:02}:{s:02},{ms:03}")
}

/// Format milliseconds as a WebVTT timestamp `HH:MM:SS.mmm`.
///
/// Mirrors `pysubs2.formats.webvtt.WebVTTFormat.ms_to_timestamp`, which reuses
/// the SubRip formatter and swaps the comma for a dot.
fn format_vtt_timestamp(ms: i64) -> String {
    format_srt_timestamp(ms).replace(',', ".")
}

/// Parse an SRT-style `HH:MM:SS,mmm` (or VTT `HH:MM:SS.mmm`) timestamp into
/// milliseconds. Hours are optional and may exceed two digits.
fn parse_timestamp(ts: &str) -> Option<i64> {
    // Accept both ',' and '.' as the millisecond separator.
    let ts = ts.trim();
    let (time_part, ms_part) = ts.rsplit_once([',', '.'])?;
    let mut fields = time_part.split(':').collect::<Vec<_>>();
    // Hours may be omitted (e.g. "00:00.420" in some VTT files).
    if fields.len() == 2 {
        fields.insert(0, "0");
    }
    if fields.len() != 3 {
        return None;
    }
    let h: i64 = fields[0].trim().parse().ok()?;
    let m: i64 = fields[1].trim().parse().ok()?;
    let s: i64 = fields[2].trim().parse().ok()?;
    // pysubs2 accepts 2- or 3-digit millis; treat a 2-digit field as hundredths.
    let ms_digits = ms_part.trim();
    let ms: i64 = ms_digits.parse().ok()?;
    let ms = if ms_digits.len() == 2 { ms * 10 } else { ms };
    Some(((h * 60 + m) * 60 + s) * 1000 + ms)
}

// ---------------------------------------------------------------------------
// SRT
// ---------------------------------------------------------------------------

/// Collapse runs of blank lines and strip leading/trailing blank lines from a
/// cue body. Mirrors `srt.make_legal_content`: `MULTI_WS_REGEX` (`\n\n+`) is
/// replaced with a single `\n` after stripping leading/trailing `\n`.
fn make_legal_content(content: &str) -> String {
    // Fast path mirrors the Python optimisation: already-legal content is
    // returned unchanged.
    if !content.is_empty() && !content.starts_with('\n') && !content.contains("\n\n") {
        return content.to_string();
    }
    let stripped = content.trim_matches('\n');
    collapse_blank_lines(stripped)
}

/// Replace every run of two-or-more `\n` with a single `\n` (`\n\n+` -> `\n`).
fn collapse_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut newline_run = 0usize;
    for ch in s.chars() {
        if ch == '\n' {
            newline_run += 1;
        } else {
            if newline_run > 0 {
                out.push('\n');
                newline_run = 0;
            }
            out.push(ch);
        }
    }
    // A trailing run of newlines collapses to a single one only if it was a run
    // of two or more would have been pre-stripped by the caller; here we mirror
    // the regex which keeps a single trailing newline if present.
    if newline_run > 0 {
        out.push('\n');
    }
    out
}

/// Decide whether `srt.sort_and_reindex` (with `skip=True`) would drop a cue:
/// empty/whitespace content, negative start, or start at-or-after end.
fn srt_should_skip(cue: &Cue) -> bool {
    cue.text.trim().is_empty() || cue.start_ms < 0 || cue.start_ms >= cue.end_ms
}

/// Parse an SRT string into cues.
///
/// Tolerant of the common deviations `srt.parse` handles: blank lines inside a
/// cue body, a missing trailing blank line, and CRLF line endings.
pub fn parse_srt(input: &str) -> Vec<Cue> {
    let normalized = input.replace("\r\n", "\n");
    let mut cues = Vec::new();

    // Split into blocks on blank-line boundaries, but reconstruct cues by
    // scanning for the "--> " timestamp arrow so blank lines inside content do
    // not break a cue apart (the look-ahead the Python regex performs).
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut i = 0usize;
    while i < lines.len() {
        // Skip blank lines between cues.
        if lines[i].trim().is_empty() {
            i += 1;
            continue;
        }

        // Optional index line: a bare integer immediately before a timestamp
        // line.
        let mut index: Option<u64> = None;
        if !lines[i].contains("-->")
            && let Ok(idx) = lines[i].trim().parse::<u64>() {
                // Only consume it as an index if the next line is a timestamp.
                if i + 1 < lines.len() && lines[i + 1].contains("-->") {
                    index = Some(idx);
                    i += 1;
                }
            }

        if i >= lines.len() || !lines[i].contains("-->") {
            i += 1;
            continue;
        }

        let Some((start_ms, end_ms)) = parse_arrow_line(lines[i]) else {
            i += 1;
            continue;
        };
        i += 1;

        // Collect content lines until the next cue (an index line followed by a
        // timestamp, or a timestamp line directly) or end of input. Blank lines
        // are retained here and legalised at compose time, matching `srt`.
        let mut content_lines: Vec<&str> = Vec::new();
        while i < lines.len() {
            if next_starts_cue(&lines, i) {
                break;
            }
            content_lines.push(lines[i]);
            i += 1;
        }
        // Trim trailing blank lines that merely separate cues.
        while content_lines.last().is_some_and(|l| l.trim().is_empty()) {
            content_lines.pop();
        }

        cues.push(Cue {
            index,
            start_ms,
            end_ms,
            text: content_lines.join("\n"),
        });
    }

    cues
}

/// Whether the line at `idx` begins a new cue: either a timestamp line, or an
/// index line whose following non-considered line is a timestamp.
fn next_starts_cue(lines: &[&str], idx: usize) -> bool {
    let line = lines[idx];
    if line.contains("-->") && parse_arrow_line(line).is_some() {
        return true;
    }
    // `index\n timestamp` look-ahead.
    if line.trim().parse::<u64>().is_ok()
        && idx + 1 < lines.len()
        && lines[idx + 1].contains("-->")
        && parse_arrow_line(lines[idx + 1]).is_some()
    {
        return true;
    }
    false
}

/// Parse a `START --> END` line into `(start_ms, end_ms)`.
fn parse_arrow_line(line: &str) -> Option<(i64, i64)> {
    let (start, end) = line.split_once("-->")?;
    Some((parse_timestamp(start)?, parse_timestamp(end)?))
}

/// Serialize cues to an SRT string, byte-identical to `srt.compose` with its
/// defaults (`reindex=True`, `strict=True`, `eol="\n"`).
pub fn compose_srt(cues: &[Cue]) -> String {
    // sort_and_reindex: sort by start, then end, then content (Subtitle
    // ordering), skip non-useful cues, renumber from 1.
    let mut ordered: Vec<&Cue> = cues.iter().collect();
    ordered.sort_by(|a, b| {
        a.start_ms
            .cmp(&b.start_ms)
            .then(a.end_ms.cmp(&b.end_ms))
            .then(a.text.cmp(&b.text))
    });

    let mut out = String::new();
    let mut number = 1u64;
    for cue in ordered {
        if srt_should_skip(cue) {
            continue;
        }
        out.push_str(&number.to_string());
        out.push('\n');
        out.push_str(&format_srt_timestamp(cue.start_ms));
        out.push_str(" --> ");
        out.push_str(&format_srt_timestamp(cue.end_ms));
        out.push('\n');
        out.push_str(&make_legal_content(&cue.text));
        out.push_str("\n\n");
        number += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// VTT (pysubs2)
// ---------------------------------------------------------------------------

/// Parse a WebVTT string into cues.
///
/// Mirrors `pysubs2` SubRip/WebVTT `from_file`: timestamp lines (two stamps on
/// one line) open a cue; following lines accumulate until the next timestamp
/// line. Text is `.strip()`-ed and a trailing next-cue index number is removed.
pub fn parse_vtt(input: &str) -> Vec<Cue> {
    let normalized = input.replace("\r\n", "\n");
    let mut timestamps: Vec<(i64, i64)> = Vec::new();
    let mut following: Vec<Vec<String>> = Vec::new();

    for line in normalized.split('\n') {
        if let Some((start, end)) = parse_arrow_line(line) {
            timestamps.push((start, end));
            following.push(Vec::new());
        } else if let Some(last) = following.last_mut() {
            // pysubs2 iterates the file keeping the line's trailing newline; we
            // join with "\n" below, so push the bare line here.
            last.push(line.to_string());
        }
    }

    timestamps
        .into_iter()
        .zip(following)
        .map(|((start, end), lines)| Cue {
            index: None,
            start_ms: start,
            end_ms: end,
            text: prepare_vtt_text(&lines),
        })
        .collect()
}

/// Reduce a cue's following lines to its text, mirroring pysubs2
/// `prepare_text`: join, `.strip()`, drop a trailing `\n+ *\d+ *$` (the index of
/// the next cue), strip unsupported HTML tags, and keep newlines (stored as
/// `\n` here rather than the SSA `\N`).
fn prepare_vtt_text(lines: &[String]) -> String {
    // "Happy empty subtitle" case: blank line(s) then a bare number line.
    if lines.len() >= 2
        && lines[..lines.len() - 1].iter().all(|l| l.trim().is_empty())
        && lines[lines.len() - 1].trim().parse::<u64>().is_ok()
    {
        return String::new();
    }

    let joined = lines.join("\n");
    let mut s = joined.trim().to_string();
    // Strip the index number of the following subtitle: `\n+ *\d+ *$`.
    s = strip_trailing_index(&s);
    // Strip any remaining HTML-ish tags (`< */? *[a-zA-Z][^>]*>`); the goldens
    // contain none, but this keeps parity with pysubs2 for general input.
    s = strip_html_tags(&s);
    s
}

/// Remove a trailing `\n+ *\d+ *$` run (the next cue's index number).
fn strip_trailing_index(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut end = bytes.len();

    // Trailing spaces.
    while end > 0 && bytes[end - 1] == b' ' {
        end -= 1;
    }
    let digits_end = end;
    while end > 0 && bytes[end - 1].is_ascii_digit() {
        end -= 1;
    }
    if end == digits_end {
        return s.to_string(); // no trailing digits
    }
    // Spaces before the digits.
    while end > 0 && bytes[end - 1] == b' ' {
        end -= 1;
    }
    // Require at least one preceding newline.
    if end == 0 || bytes[end - 1] != b'\n' {
        return s.to_string();
    }
    while end > 0 && bytes[end - 1] == b'\n' {
        end -= 1;
    }
    s[..end].to_string()
}

/// Strip HTML-style tags matching pysubs2's `< */? *[a-zA-Z][^>]*>`.
fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<'
            && let Some(close) = find_tag_close(bytes, i) {
                i = close + 1;
                continue;
            }
        // Copy one UTF-8 char.
        let ch_len = utf8_len(bytes[i]);
        out.push_str(&s[i..i + ch_len]);
        i += ch_len;
    }
    out
}

/// Match `< */? *[a-zA-Z][^>]*>` starting at `start` (a `<`); return the index
/// of the closing `>` if it matches.
fn find_tag_close(bytes: &[u8], start: usize) -> Option<usize> {
    let mut j = start + 1;
    while j < bytes.len() && bytes[j] == b' ' {
        j += 1;
    }
    if j < bytes.len() && bytes[j] == b'/' {
        j += 1;
        while j < bytes.len() && bytes[j] == b' ' {
            j += 1;
        }
    }
    if j >= bytes.len() || !bytes[j].is_ascii_alphabetic() {
        return None;
    }
    // `[^>]*>`
    while j < bytes.len() {
        if bytes[j] == b'>' {
            return Some(j);
        }
        j += 1;
    }
    None
}

fn utf8_len(first: u8) -> usize {
    match first {
        b if b < 0x80 => 1,
        b if b >> 5 == 0b110 => 2,
        b if b >> 4 == 0b1110 => 3,
        _ => 4,
    }
}

/// Serialize cues to a WebVTT string, byte-identical to pysubs2
/// `SSAFile.to_string("vtt")`.
///
/// Emits the `WEBVTT\n\n` header, then for each visible cue (sorted by start)
/// a 1-based number, the dot-separated timestamp line, and the text with blank
/// lines collapsed and surrounding whitespace stripped.
pub fn compose_vtt(cues: &[Cue]) -> String {
    let mut ordered: Vec<&Cue> = cues.iter().collect();
    // WebVTTFormat._get_visible_lines sorts by start (stable).
    ordered.sort_by_key(|c| c.start_ms);

    let mut out = String::from("WEBVTT\n\n");
    for (n, cue) in ordered.iter().enumerate() {
        let lineno = n + 1;
        out.push_str(&lineno.to_string());
        out.push('\n');
        out.push_str(&format_vtt_timestamp(cue.start_ms));
        out.push_str(" --> ");
        out.push_str(&format_vtt_timestamp(cue.end_ms));
        out.push('\n');
        // prepare_text: collapse `\n+` -> `\n`, then strip.
        let text = collapse_blank_lines(cue.text.trim()).trim().to_string();
        out.push_str(&text);
        out.push_str("\n\n");
    }
    out
}

#[cfg(test)]
mod parity {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn fixtures_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/subtitle")
    }

    /// Each `*.srt` golden is the output of `srt.parse` -> `srt.compose`. Our
    /// `parse_srt` -> `compose_srt` must re-emit it byte-for-byte.
    #[test]
    fn srt_roundtrip() {
        let dir = fixtures_dir();
        let mut checked = 0;
        for entry in fs::read_dir(&dir).expect("read fixtures/subtitle") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("srt") {
                continue;
            }
            let golden = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            let cues = parse_srt(&golden);
            let emitted = compose_srt(&cues);
            assert_eq!(
                emitted,
                golden,
                "SRT round-trip mismatch for {}",
                path.display()
            );
            checked += 1;
        }
        assert!(checked > 0, "no .srt goldens found in {}", dir.display());
    }

    /// Each `*.vtt` golden is the output of pysubs2 `from_string` ->
    /// `to_string`. Our `parse_vtt` -> `compose_vtt` must re-emit it byte-for-byte.
    #[test]
    fn vtt_roundtrip() {
        let dir = fixtures_dir();
        let mut checked = 0;
        for entry in fs::read_dir(&dir).expect("read fixtures/subtitle") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("vtt") {
                continue;
            }
            let golden = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            let cues = parse_vtt(&golden);
            let emitted = compose_vtt(&cues);
            assert_eq!(
                emitted,
                golden,
                "VTT round-trip mismatch for {}",
                path.display()
            );
            checked += 1;
        }
        assert!(checked > 0, "no .vtt goldens found in {}", dir.display());
    }

    #[test]
    fn srt_timestamp_format() {
        assert_eq!(format_srt_timestamp(0), "00:00:00,000");
        assert_eq!(format_srt_timestamp(5_500), "00:00:05,500");
        assert_eq!(format_srt_timestamp(8_250), "00:00:08,250");
        // 1h23m4s
        assert_eq!(format_srt_timestamp(4_984_000), "01:23:04,000");
    }

    #[test]
    fn vtt_timestamp_format() {
        assert_eq!(format_vtt_timestamp(1_000), "00:00:01.000");
        assert_eq!(format_vtt_timestamp(8_250), "00:00:08.250");
    }

    #[test]
    fn parse_then_inspect_srt() {
        let golden = fs::read_to_string(fixtures_dir().join("basic.srt")).unwrap();
        let cues = parse_srt(&golden);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].index, Some(1));
        assert_eq!(cues[0].start_ms, 1_000);
        assert_eq!(cues[0].end_ms, 4_000);
        assert_eq!(cues[0].text, "Hello, world!");
        assert_eq!(cues[1].text, "Second line\nand a wrap.");
    }

    #[test]
    fn parse_then_inspect_vtt() {
        let golden = fs::read_to_string(fixtures_dir().join("basic.vtt")).unwrap();
        let cues = parse_vtt(&golden);
        assert_eq!(cues.len(), 2);
        // VTT cues carry no index of their own; pysubs2 numbers on output.
        assert_eq!(cues[0].index, None);
        assert_eq!(cues[0].start_ms, 1_000);
        assert_eq!(cues[1].text, "Second line\nand a wrap.");
    }

    /// `compose_srt` reindexes from 1 by start time regardless of input order
    /// or source indices, mirroring `srt.compose`'s default `reindex=True`.
    #[test]
    fn compose_srt_reindexes_by_start() {
        let cues = vec![
            Cue {
                index: Some(999),
                start_ms: 2_000,
                end_ms: 3_000,
                text: "second".into(),
            },
            Cue {
                index: Some(0),
                start_ms: 1_000,
                end_ms: 2_000,
                text: "first".into(),
            },
        ];
        let out = compose_srt(&cues);
        assert_eq!(
            out,
            "1\n00:00:01,000 --> 00:00:02,000\nfirst\n\n\
             2\n00:00:02,000 --> 00:00:03,000\nsecond\n\n"
        );
    }

    /// `srt.compose` (skip=True) drops empty cues and zero/negative-length cues.
    #[test]
    fn compose_srt_skips_non_useful() {
        let cues = vec![
            Cue {
                index: Some(1),
                start_ms: 0,
                end_ms: 1_000,
                text: "   ".into(), // whitespace only -> skipped
            },
            Cue {
                index: Some(2),
                start_ms: 1_000,
                end_ms: 1_000,
                text: "zero length".into(), // start == end -> skipped
            },
            Cue {
                index: Some(3),
                start_ms: 2_000,
                end_ms: 3_000,
                text: "kept".into(),
            },
        ];
        let out = compose_srt(&cues);
        assert_eq!(out, "1\n00:00:02,000 --> 00:00:03,000\nkept\n\n");
    }

    /// `make_legal_content` collapses blank lines within cue text.
    #[test]
    fn compose_srt_legalises_blank_lines() {
        let cues = vec![Cue {
            index: Some(1),
            start_ms: 0,
            end_ms: 1_000,
            text: "a\n\nb".into(),
        }];
        let out = compose_srt(&cues);
        assert_eq!(out, "1\n00:00:00,000 --> 00:00:01,000\na\nb\n\n");
    }

    #[test]
    fn parse_handles_crlf() {
        let input = "1\r\n00:00:00,000 --> 00:00:01,000\r\nhi\r\n\r\n";
        let cues = parse_srt(input);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "hi");
        assert_eq!(compose_srt(&cues), "1\n00:00:00,000 --> 00:00:01,000\nhi\n\n");
    }
}
