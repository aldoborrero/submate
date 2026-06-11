"""Capture subtitle translation with a deterministic stubbed LLM.

Falsifier targets: submate-translate parity::{chunking,apply}.

The LLM is stubbed to an IDENTITY echo (returns the prompt's text payload
unchanged). The point of these fixtures is to pin the *plumbing* — prompt
construction, chunk boundaries, separator join/split, and re-application onto
cues — not translation quality. The Rust translate test serves `mock_llm.json`
(recorded prompt -> completion pairs) via wiremock, so the Rust port must build
byte-identical prompts and apply the completions identically to pass.

Emits:
  translate/sampleA.in.srt   — input (also written here for the Rust test)
  translate/sampleA.out.srt  — Python-produced output (golden)
  translate/mock_llm.json    — {prompt: completion} the stub recorded
  translate/chunking.json    — the ordered `combined` strings sent per batch
"""

from __future__ import annotations

import os

from _common import FIXTURES, write_json, write_text

SAMPLE_SRT = """\
1
00:00:01,000 --> 00:00:03,000
Hello, world.

2
00:00:03,500 --> 00:00:05,000
This is a test subtitle.

3
00:00:05,500 --> 00:00:07,000
Goodbye.
"""

# Marker the shared prompt template ends with: "...Text:\n{text}".
PAYLOAD_MARKER = "Text:\n"


def main() -> None:
    os.environ.setdefault("SUBMATE__TRANSLATION__BACKEND", "ollama")
    from submate.config import Config
    from submate.translation import TranslationService

    service = TranslationService(Config())

    recorded: dict[str, str] = {}
    chunks: list[str] = []

    def fake_complete(prompt: str) -> str:
        # identity: echo the text payload back unchanged
        payload = prompt.split(PAYLOAD_MARKER, 1)[-1]
        recorded[prompt] = payload
        return payload

    orig_translate = service.backend.translate

    def wrapped_translate(text, source_lang, target_lang, prompt_template=None):
        chunks.append(text)
        return orig_translate(text, source_lang, target_lang, prompt_template=prompt_template)

    service.backend._complete = fake_complete  # type: ignore[method-assign]
    service.backend.translate = wrapped_translate  # type: ignore[method-assign]

    out_srt = service.translate_srt_content(SAMPLE_SRT, "en", "es")

    write_text("translate/sampleA.in.srt", SAMPLE_SRT)
    write_text("translate/sampleA.out.srt", out_srt)
    write_json("translate/mock_llm.json", recorded)
    write_json("translate/chunking.json", {"batches": chunks})
    print(f"captured {len(recorded)} llm calls, {len(chunks)} batches -> {FIXTURES / 'translate'}")


if __name__ == "__main__":
    main()
