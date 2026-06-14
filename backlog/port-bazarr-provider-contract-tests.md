# Bazarr Whisper-provider compatibility test suite

**blocked-by:** port-server-bazarr-asr

## what

A focused integration suite that pins submate to the **real** behavior of
Bazarr's Whisper provider (`custom_libs/subliminal_patch/providers/
whisperai.py`), so a future change can't silently break the integration. The
suite drives `app()` with a **fake `BazarrTranscriber`** (no model) and asserts
the exact request/response shape the provider produces and consumes.

Pin each of these provider facts as a named test:

1. **Raw-PCM upload** — the provider posts `encode=false` with multipart field
   `audio_file` that is raw s16le/mono/16k PCM (ffmpeg `format="s16le"`, no WAV
   header). Assert the route accepts a multipart `audio_file` of raw PCM and the
   bytes reach the transcriber unwrapped.
2. **`/asr` params** — `task ∈ {transcribe,translate}`, `language=<alpha2>`,
   `output=srt`, `encode=false`, `video_file`. Assert they parse to the right
   `JobOpts` (and `output=srt` is what the provider always sends).
3. **SRT body + `Source` header** — success returns the subtitle as the response
   **body** (the provider does `subtitle.content = r.content`) with
   `Source: Transcribed using stable-ts from Submate`.
4. **No error body on `/asr`** — provider never checks `r.status_code`; assert a
   transcriber error yields an **empty** body (subliminal discards → retry),
   NOT a JSON `{"detail":...}` that would be saved as a corrupt subtitle.
5. **`/detect-language` JSON + error tolerance** — success returns
   `{"language_code","detected_language"}` (provider reads `r.json()`); any error
   returns **200** `{"Unknown","und"}` (provider maps non-conforming → `None`).
6. **Concurrency cap is shared** — N concurrent `/asr` requests against a fake
   whose `transcribe` blocks on a barrier observe at most `runners` in flight at
   once (the `Dispatcher` permit bounds Bazarr the same as the queue drain), and
   the rest **wait** rather than erroring.

## where

`rust/crates/submate-server/tests/bazarr_provider.rs` (new integration test),
using `axum`'s `oneshot`/`tower::ServiceExt` against `app()` with a fake
transcriber. No `model` feature, no network, no fixtures.

## why

The user requirement is "works reliably with the Whisper provider." These tests
encode the four provider behaviors that are easy to regress and would silently
corrupt or drop subtitles in production (raw-PCM not WAV, SRT-in-body, never an
error body on `/asr`, 200-on-detect-failure) plus the shared concurrency bound
that protects against a whole-TV-show burst.

## falsifies

`cargo test -p submate-server --test bazarr_provider` — all of the named tests
above green under the nix-devshell `fastCheck`.
