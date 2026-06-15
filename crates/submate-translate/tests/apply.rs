//! Parity: the mocked-LLM SRT apply flow matches the golden EXACTLY.
//!
//! This is the "mocked-LLM translation must match the golden byte-for-byte"
//! layer that `parity::chunking` does not cover. `chunking` stops at the joined
//! batch string; this test drives the *whole*
//! [`submate_translate::translate_srt_content`] flow, with completions served
//! from `fixtures/translate/mock_llm.json` by exact prompt key (no HTTP), and
//! asserts the recomposed SRT equals `fixtures/translate/sampleA.out.srt`.
//!
//! Flow (en -> es, so the `source_lang == target_lang` short-circuit is
//! NOT exercised):
//! 1. `translate_srt_content` parses `sampleA.in.srt` into cues;
//! 2. default `chunk_size = 50`, so the 3 cues form a single batch;
//! 3. the cue contents are joined with `SRT_SEPARATOR_TOKEN`
//!    (`"\n---BREAK---\n".join(texts)`) and formatted into `TRANSLATION_PROMPT`;
//! 4. the test's `complete` closure looks the prompt up in `mock_llm.json`
//!    (exact-key, no HTTP) and returns the recorded completion;
//! 5. `split_batch` realigns-or-keeps-originals;
//! 6. cues are re-emitted preserving index/timing with replaced content and
//!    recomposed to an SRT string.
//!
//! The committed `mock_llm.json` records an IDENTITY-echo stub: the completion
//! equals the full prompt. That prompt contains the literal `---BREAK---` in
//! its Rules line plus the two batch separators, so splitting the completion
//! yields 4 blocks for 3 inputs — a count mismatch. `split_batch` therefore
//! keeps the originals, and the golden output's cue text equals the input's.
//! The load-bearing parity here is the SRT re-compose: cue re-indexing and the
//! exact byte layout (blank-line separators plus `srt.compose`'s trailing
//! newline) all have to match.

use std::convert::Infallible;

use submate_translate::translate_srt_content;

/// Default `chunk_size`.
const DEFAULT_CHUNK_SIZE: usize = 50;

mod parity {
    use super::*;

    /// Falsifier `cargo test -p submate-translate parity::apply`: the
    /// mocked-LLM SRT apply flow reproduces the golden byte-for-byte.
    #[tokio::test]
    async fn apply() {
        let input = std::fs::read_to_string(::parity::fixture_path("translate/sampleA.in.srt"))
            .expect("missing translate/sampleA.in.srt fixture");

        // Recorded `{prompt: completion}` pairs (exact-key lookup, no HTTP).
        let mock = ::parity::golden("translate/mock_llm.json");
        let completions = mock
            .as_object()
            .expect("mock_llm.json is not a JSON object")
            .clone();

        // The mocked async "backend": return the recorded completion for the
        // exact prompt the apply layer builds. Lookup failure is a test bug, so
        // panic.
        let mut complete = async |prompt: String| -> Result<String, Infallible> {
            let completion = completions
                .get(&prompt)
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("no recorded completion for prompt:\n{prompt}"));
            Ok(completion.to_string())
        };

        let actual = translate_srt_content(&input, "en", "es", DEFAULT_CHUNK_SIZE, &mut complete)
            .await
            .unwrap();

        let golden = std::fs::read_to_string(::parity::fixture_path("translate/sampleA.out.srt"))
            .expect("missing translate/sampleA.out.srt fixture");
        ::parity::assert_str_eq(&actual, &golden);
    }
}
