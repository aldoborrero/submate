# Bazarr ASR + detect-language routes (direct dispatch, no queue)

**blocked-by:** port-bazarr-direct-core, align-bazarr-route-signatures

> Rewritten from the enqueue-and-await design: Bazarr is now a **direct,
> semaphore-bounded transcription** (see `port-bazarr-direct-core` design note).
> The routes call the `BazarrTranscriber` seam, NOT `NodeCoordinator::enqueue`.

## what

Wire the two real Bazarr routes in `submate-server` to the `BazarrTranscriber`
seam and mount them (today `bazarr_router()` is an empty `Router::new()` and
`submate-bazarr` is not even a server dep).

- **`AppState` gains** `bazarr: Option<Arc<dyn BazarrTranscriber>>`. When absent
  (brain-only server, no compute), `/asr` returns an **empty body** and
  `/detect-language` returns the `Unknown`/`und` JSON — never an error envelope.
- **`POST /bazarr/asr`** — read the raw request body as `Bytes` (the s16le PCM
  Bazarr uploads with `encode=false`), parse the query params per
  `align-bazarr-route-signatures`, build a `JobOpts` (`task`, auto-detect source,
  `output_format` from `output`, `target_language` from `language`,
  `word_timestamps`), call `bazarr.transcribe(&opts, pcm).await`, and stream the
  resulting subtitle as `text/plain` with header
  **`Source: Transcribed using stable-ts from Submate`**.
- **`POST /bazarr/detect-language`** — read the PCM body, call
  `bazarr.detect(pcm).await`, map the iso-639-1 result to its display name
  (`port-bazarr-language-name-lookup`), and return JSON
  `{"detected_language","language_code"}`.
- Wire `cmd_server` (`submate-cli`) to construct the production
  `BazarrTranscriber` from the **embedded node's `Dispatcher`** (the same handle
  the embedded node uses, so Bazarr and the queue drain share one runner cap) +
  the configured model path, and pass it into `AppState`.

## CRITICAL contract (reliability — Bazarr never checks `r.status_code` on /asr)

`download_subtitle` does `subtitle.content = r.content` with **no status check**
(verified in the provider source `whisperai.py`). Therefore `/bazarr/asr`:

- **success** → SRT bytes in the body, `Source` header, 200.
- **any failure** (no compute, transcribe error, bad input) → **empty body**, so
  subliminal discards it as "no subtitle" and Bazarr retries on its next
  scheduled pass. **Never** return a JSON/text error body on `/asr` — it would be
  saved as a corrupt subtitle. (The single exception is `ValueError` on an
  invalid `output` value → 400 `detail=str(e)`, per the align item; subliminal
  only hits it on misconfiguration, never mid-show.)
- **backpressure under load** → the `Dispatcher` permit is acquired *inside*
  `transcribe`; a busy server **waits** for a runner (Bazarr's transcription
  timeout is large by design). Do NOT fast-`503` a non-empty body.

`/bazarr/detect-language` is the opposite — fully error-tolerant: **any** failure
returns **200** `{"detected_language":"Unknown","language_code":"und"}`.

## where

`rust/crates/submate-server/src/lib.rs` (`AppState`, `bazarr_router`, `app()`),
`rust/crates/submate-cli/src/main.rs` (`cmd_server` wiring + the production
`BazarrTranscriber`). Add `submate-bazarr` to `submate-server`'s `Cargo.toml`.

## why

Makes submate a working Bazarr Whisper provider, the primary production
interface, while keeping the durable queue off the synchronous path.

## falsifies

`cargo test -p submate-server bazarr_routes::*` against `app()` with a **fake**
`BazarrTranscriber` (no model):

- `bazarr_routes::asr_returns_srt_body_with_source_header` — `POST /bazarr/asr`
  with a PCM body + `output=srt` returns the fake's content as the body with the
  exact `Source` header, 200.
- `bazarr_routes::asr_failure_returns_empty_body` — when the fake errors (or
  `bazarr` is `None`), the response body is **empty** and is NOT a JSON/text
  error envelope.
- `bazarr_routes::detect_returns_json` — `POST /bazarr/detect-language` returns
  `{"detected_language","language_code"}` from the fake.
- `bazarr_routes::detect_failure_is_200_unknown` — a fake `detect` error yields
  **200** `{"detected_language":"Unknown","language_code":"und"}`.
