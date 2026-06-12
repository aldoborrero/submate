//! Parity: the mocked-LLM SRT apply flow matches the Python golden EXACTLY.
//!
//! This is the "mocked-LLM translation must match Python byte-for-byte" layer
//! that `parity::chunking` does not cover. `chunking` stops at the joined batch
//! string; this test drives the *whole* `TranslationService.translate_srt_content`
//! flow (`submate/translation.py`) with completions served from
//! `rust/fixtures/translate/mock_llm.json` by exact prompt key (no HTTP), and
//! asserts the recomposed SRT equals `rust/fixtures/translate/sampleA.out.srt`.
//!
//! Flow ported (en -> es, so the `source_lang == target_lang` short-circuit is
//! NOT exercised):
//! 1. parse `sampleA.in.srt` into cues (index/start/end/content);
//! 2. default `chunk_size = 50`, so the 3 cues form a single batch;
//! 3. `combined = join_batch(contents, SRT_SEPARATOR_TOKEN)`
//!    (`"\n---BREAK---\n".join(texts)`);
//! 4. `prompt = format_prompt(TRANSLATION_PROMPT, "en", "es", combined)`;
//! 5. `completion = mock_llm.json[prompt]` (exact-key lookup);
//! 6. `parts = split_batch(completion, SRT_SEPARATOR_TOKEN, &contents)` —
//!    realign-or-keep-originals;
//! 7. re-emit cues preserving index/start/end with replaced content;
//! 8. recompose to an SRT string.
//!
//! The committed `mock_llm.json` records an IDENTITY-echo stub: the completion
//! equals the full prompt. That prompt contains the literal `---BREAK---` in
//! its Rules line plus the two batch separators, so splitting the completion
//! yields 4 blocks for 3 inputs — a count mismatch. `split_batch` therefore
//! keeps the originals, and the golden output's cue text equals the input's.
//! The load-bearing parity here is the SRT re-compose: cue re-indexing and the
//! exact byte layout (blank-line separators plus `srt.compose`'s trailing
//! newline) all have to match.

use submate_translate::{
    chunk_ranges, format_prompt, join_batch, split_batch, SRT_SEPARATOR_TOKEN, TRANSLATION_PROMPT,
};

/// Python default `chunk_size` (`TranslationSettings.chunk_size`).
const DEFAULT_CHUNK_SIZE: usize = 50;

/// A parsed SRT cue (mirrors the fields `srt.Subtitle` carries through
/// `translate_subtitles`: index, timing, content).
struct Cue {
    index: u32,
    timing: String,
    content: String,
}

/// Test-local SRT reader (the shipped SRT parser is a separate port item, and
/// the backlog item explicitly permits a documented test-local reader exactly
/// as `chunking.rs` already does). SRT blocks are separated by blank lines;
/// within a block the first line is the index, the second the timing, and the
/// remainder is the cue content. Sufficient for the committed single-line
/// `sampleA.in.srt` fixture.
fn parse_srt(srt: &str) -> Vec<Cue> {
    let normalized = srt.replace("\r\n", "\n");
    normalized
        .split("\n\n")
        .filter_map(|block| {
            let lines: Vec<&str> = block.lines().collect();
            if lines.len() < 3 {
                return None;
            }
            Some(Cue {
                index: lines[0].trim().parse().expect("SRT index is not a number"),
                timing: lines[1].to_string(),
                content: lines[2..].join("\n"),
            })
        })
        .collect()
}

/// Test-local SRT writer reproducing Python `srt.compose`: each cue is
/// `index\ntiming\ncontent\n`, blocks separated (and the file terminated) by a
/// blank line — i.e. every cue, including the last, is followed by `\n\n`. This
/// matches the trailing blank line in the golden `sampleA.out.srt`.
fn compose_srt(cues: &[Cue]) -> String {
    let mut out = String::new();
    for cue in cues {
        out.push_str(&cue.index.to_string());
        out.push('\n');
        out.push_str(&cue.timing);
        out.push('\n');
        out.push_str(&cue.content);
        out.push_str("\n\n");
    }
    out
}

mod parity {
    use super::*;

    /// Falsifier `cargo test -p submate-translate parity::apply`: the
    /// mocked-LLM SRT apply flow reproduces the Python golden byte-for-byte.
    #[test]
    fn apply() {
        let input = std::fs::read_to_string(::parity::fixture_path("translate/sampleA.in.srt"))
            .expect("missing translate/sampleA.in.srt fixture");
        let cues = parse_srt(&input);

        // Recorded `{prompt: completion}` pairs (exact-key lookup, no HTTP).
        let mock = ::parity::golden("translate/mock_llm.json");
        let completions = mock
            .as_object()
            .expect("mock_llm.json is not a JSON object");

        // Default chunk_size keeps all 3 cues in one batch, but drive the
        // real chunk loop so the port matches `translate_subtitles` exactly.
        let mut translated: Vec<Cue> = Vec::with_capacity(cues.len());
        for range in chunk_ranges(cues.len(), DEFAULT_CHUNK_SIZE) {
            let batch = &cues[range];
            let contents: Vec<String> = batch.iter().map(|c| c.content.clone()).collect();

            let combined = join_batch(&contents, SRT_SEPARATOR_TOKEN);
            let prompt = format_prompt(TRANSLATION_PROMPT, "en", "es", &combined);
            let completion = completions
                .get(&prompt)
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("no recorded completion for prompt:\n{prompt}"));

            let parts = split_batch(completion, SRT_SEPARATOR_TOKEN, &contents);
            assert_eq!(
                parts.len(),
                batch.len(),
                "split_batch must preserve batch length"
            );

            for (cue, content) in batch.iter().zip(parts) {
                translated.push(Cue {
                    index: cue.index,
                    timing: cue.timing.clone(),
                    content,
                });
            }
        }

        let actual = compose_srt(&translated);
        let golden = std::fs::read_to_string(::parity::fixture_path("translate/sampleA.out.srt"))
            .expect("missing translate/sampleA.out.srt fixture");
        ::parity::assert_str_eq(&actual, &golden);
    }
}
