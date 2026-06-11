//! Parity: the chunking machinery reproduces the Python-recorded batch strings.
//!
//! `rust/fixtures/translate/chunking.json` records, in order, every `combined`
//! string the Python `TranslationService` passed to `backend.translate` while
//! translating `sampleA.in.srt` with the default `chunk_size`. Each entry is
//! `("\n---BREAK---\n").join(cue_contents_for_that_batch)`. This test takes the
//! same cue contents, runs them through [`chunk_ranges`] + [`join_batch`], and
//! asserts byte-for-byte equality against the golden `batches`.

use submate_translate::{chunk_ranges, join_batch, SRT_SEPARATOR_TOKEN};

/// Python default `chunk_size` (`TranslationSettings.chunk_size`).
const DEFAULT_CHUNK_SIZE: usize = 50;

/// Extract SRT cue contents from a minimal SRT string.
///
/// Test-local reader (the shipped SRT parser is a separate port item): SRT
/// blocks are separated by blank lines; within a block the first line is the
/// index, the second the timing, and the remainder is the cue content. This is
/// sufficient for the committed `sampleA.in.srt` fixture, whose cues are single
/// lines.
fn srt_cue_contents(srt: &str) -> Vec<String> {
    let normalized = srt.replace("\r\n", "\n");
    normalized
        .split("\n\n")
        .filter_map(|block| {
            let lines: Vec<&str> = block.lines().collect();
            if lines.len() < 3 {
                return None;
            }
            Some(lines[2..].join("\n"))
        })
        .collect()
}

mod parity {
    use super::*;

    /// Falsifier `cargo test -p submate-translate parity::chunking`: the
    /// chunk boundaries + separator-token joins match the Python golden.
    #[test]
    fn chunking() {
        let srt = std::fs::read_to_string(::parity::fixture_path("translate/sampleA.in.srt"))
            .expect("missing translate/sampleA.in.srt fixture");
        let contents = srt_cue_contents(&srt);

        let batches: Vec<String> = chunk_ranges(contents.len(), DEFAULT_CHUNK_SIZE)
            .into_iter()
            .map(|range| join_batch(&contents[range], SRT_SEPARATOR_TOKEN))
            .collect();

        let golden = ::parity::golden("translate/chunking.json");
        let golden_batches: Vec<&str> = golden["batches"]
            .as_array()
            .expect("chunking.json missing `batches` array")
            .iter()
            .map(|v| v.as_str().expect("batch entry is not a string"))
            .collect();

        assert_eq!(
            batches.len(),
            golden_batches.len(),
            "batch count {} != golden {}",
            batches.len(),
            golden_batches.len()
        );
        for (actual, expected) in batches.iter().zip(golden_batches) {
            ::parity::assert_str_eq(actual, expected);
        }
    }
}
