# Bazarr direct-dispatch core: transcribe seam + orchestration (no queue)

**blocked-by:** port-bazarr-pcm-to-f32, port-bazarr-translate-content-format-dispatch

> **Design note (supersedes `port-queue-bazarr-service`).** Bazarr's Whisper
> provider is *synchronous* — it holds the HTTP connection per file and reads
> the subtitle from the response body (`custom_libs/subliminal_patch/providers/
> whisperai.py`, `download_subtitle`). Its audio is ephemeral in-RAM PCM, so a
> durable queue can neither add durability nor deliver a result after Bazarr's
> connection drops. Bazarr is therefore handled by a **direct, semaphore-bounded
> transcription**, NOT the durable `submate-queue`. The queue stays for the
> async file-backed paths (scan, Jellyfin). See `rust/docs/architecture.md`.

## what

Two pieces, both in `submate-bazarr` + a thin seam the server holds:

1. **An object-safe transcribe seam** so the server can run a transcription
   directly without the node-coordination queue, and so the routes are testable
   without loading a model:

   ```rust
   /// One synchronous Bazarr transcription. The production impl wraps the
   /// embedded node's `Dispatcher` (the shared semaphore that also bounds the
   /// queue drain) + whisper; tests inject a fake returning a canned result.
   pub trait BazarrTranscriber: Send + Sync {
       fn transcribe<'a>(
           &'a self,
           opts: &'a JobOpts,          // submate-proto: model, task, source/target lang, output_format
           pcm: Vec<u8>,               // raw s16le/mono/16k from Bazarr (encode=false)
       ) -> Pin<Box<dyn Future<Output = Result<BazarrOutput, String>> + Send + 'a>>;

       /// Detect the spoken language of a clip (first ~30 s is enough).
       fn detect<'a>(&'a self, pcm: Vec<u8>)
           -> Pin<Box<dyn Future<Output = Result<String /*iso-639-1*/, String>> + Send + 'a>>;
   }
   pub struct BazarrOutput { pub content: String, pub detected_language: String }
   ```

2. **The orchestration** that the production `BazarrTranscriber` performs, lifted
   from `BazarrService.transcribe_audio_bytes` / `detect_language`
   (`submate/queue/services/bazarr.py`):
   - `pcm_s16le_to_f32(&pcm)` (from `port-bazarr-pcm-to-f32`) → `Vec<f32>`.
   - run whisper under a **`Dispatcher` permit** (`Dispatcher::transcribe_pcm`,
     held across inference so Bazarr shares the runner cap with the queue drain).
     Source language is **auto-detected** (`language=None`) and `task` honored —
     mirror Python: it ignores the provider's `language` query param as a decode
     hint and uses it as the *target* for the LLM-translate step below.
   - `assemble_result` → render per `output_format`: SRT/VTT via
     `to_srt_vtt(word_level=word_timestamps[, vtt])`, TXT via `.text`, JSON via
     `to_dict` (`submate-whisper`/`stable-ts`, already ported).
   - **translate-if-target**: when `opts.target_language` is `Some(t)` and
     `t != detected_language`, run `translate_content` (from
     `port-bazarr-translate-content-format-dispatch`); on any translate error,
     return the *untranslated* content (Python `_translate_content` fallback).
   - **detect**: cap to the first 30 s of f32 samples
     (`16_000 * 30 = 480_000`), transcribe, return `result.language` (or `"und"`).

## where

`rust/crates/submate-bazarr/src/` — the `BazarrTranscriber` trait, `BazarrOutput`,
the format-dispatch orchestration, and the production impl wrapping a
`submate_node::Dispatcher` + model path (model-gated, like
`submate_node::whisper_processor`). `submate-bazarr` gains deps on
`submate-proto`, `submate-whisper`, `submate-node`, `submate-translate`,
`submate-queue` (for `OutputFormat`) — all already in the workspace.

## why

This is the seam that lets Bazarr be a direct synchronous transcription
(correct per the provider contract) while sharing one concurrency limiter with
the rest of the system, and the boundary that makes the routes testable with a
fake (no model in the gate).

## falsifies

`cargo test -p submate-bazarr direct::*` (no `model` feature; a fake
`BazarrTranscriber` is NOT needed here — these test the pure orchestration via a
fake whisper-result closure):

- `direct::format_dispatch` — given a canned assembled result, the orchestration
  emits SRT / VTT / TXT / JSON for the matching `output_format` (byte-equal to
  the stable-ts renderers).
- `direct::translate_only_when_target_differs` — `target=None` and
  `target==detected` return content unchanged (no translate call); a differing
  target invokes the (mocked) translate path; a translate error returns the
  untranslated content.
- `direct::detect_caps_to_30s` — `detect` truncates its f32 input to
  `≤ 480_000` samples before inference (assert the slice length handed to the
  fake whisper closure).
