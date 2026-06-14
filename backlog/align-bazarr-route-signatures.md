# align: Bazarr ASR + detect-language route signatures

**relates-to:** port-server-bazarr-asr

## what

Pin the byte-for-byte HTTP contract of the two Bazarr routes so the
`port-server-bazarr-asr` implementer wires the exact param names, defaults,
patterns, response keys, status codes, and headers that Bazarr's Whisper
provider expects. The existing `port-server-bazarr-asr.md` falsifier only proves
the enqueue/await *mechanics* (`bazarr_asr_enqueues`) — it does not pin the
request/response *shape*, which is a separate route-signature contract that can
silently drift (a renamed query param, a missing pattern, a wrong header, the
detect-language error path returning a non-200).

Spec is `submate/server/handlers/bazarr/router.py` (route decorators + `Query`
defaults/patterns), `handlers.py`, and `models.py`. Both routes live under the
`/bazarr` router prefix.

### `POST /bazarr/asr`

Query params (all from FastAPI `Query`, names are wire-exact):

| param | type | default | constraint |
|-------|------|---------|------------|
| `task` | str | `"transcribe"` | pattern `^(transcribe\|translate)$` |
| `language` | str? | `None` | — |
| `output` | str | `"srt"` | pattern `^(srt\|vtt\|txt\|json)$` |
| `encode` | bool | `true` | accepted but ignored (`_ = encode`) |
| `word_timestamps` | bool | `false` | — |
| `video_file` | str? | `None` | — |

Body: the uploaded audio (Python `audio_file: UploadFile = File(...)`; the Rust
port relays a raw PCM `Bytes` body — see `port-server-bazarr-asr`).

Success response: the subtitle text as `text/plain` with response header
**`Source: Transcribed using stable-ts from Submate`** (exact string).

**Failure response (corrects the Python spec — see `port-bazarr-provider-contract-tests`).**
Bazarr's provider does `subtitle.content = r.content` with **no status check**
(`whisperai.py::download_subtitle`), so an error body would be saved as a corrupt
subtitle. The Python `/asr` returns a 500 with `detail="Transcription failed"`,
which is therefore a *bug* against the real client. The Rust port instead returns
an **empty body** on any transcription failure (so subliminal discards it →
Bazarr retries on its schedule) and **never** an error envelope. The one 400 case
the Python keeps — `ValueError` on an invalid `output` value — is acceptable
because subliminal only hits it on misconfiguration, never mid-show; do NOT mimic
FastAPI's 422 query-validation envelope (a framework artifact no Bazarr request
triggers).

### `POST /bazarr/detect-language`

Query params:

| param | type | default | constraint |
|-------|------|---------|------------|
| `encode` | bool | `true` | accepted, ignored |
| `detect_lang_length` | int | `30` | `ge=1, le=300` |
| `detect_lang_offset` | int | `0` | `ge=0` |
| `video_file` | str? | `None` | — |

Response model `LanguageDetectionResponse` (`models.py`), JSON keys wire-exact:

```json
{ "detected_language": "<human-readable name>", "language_code": "<iso-639-1>" }
```

**Error path is Bazarr-compatible: status stays 200.** Any exception during
detection returns `{"detected_language": "Unknown", "language_code": "und"}`
(see `router.py` — the `except Exception` branch returns the model, it does NOT
raise an `HTTPException`). An implementer who maps detection failure to a 4xx/5xx
breaks Bazarr's auto-detect flow.

## where

`rust/crates/submate-server/src/lib.rs` (route handlers) and
`rust/crates/submate-bazarr/src/lib.rs` (the `LanguageDetectionResponse` struct
— it does not exist yet; it must serialize to exactly the two keys above).

## why

These are route-signature contracts (param names/defaults/patterns, response
keys, status codes, the `Source` header). They must match Python byte-for-byte
or Bazarr's Whisper provider rejects the response. Unlike the Jellyfin webhook
route — already pinned by `jellyfin_webhook_route_mounted_at_webhooks_jellyfin`
in `submate-server` — there is no test or golden fixture pinning the Bazarr
route shape today.

## falsifies

Two `cargo test -p submate-server` cases (added alongside the
`port-server-bazarr-asr` wiring):

1. `bazarr_asr_response_headers` — a successful `/bazarr/asr` POST returns
   `content-type: text/plain` and header
   `Source: Transcribed using stable-ts from Submate`; a transcription failure
   yields an **empty body** (NOT a 500/400 with an error envelope — that would be
   saved as a corrupt subtitle). An invalid `output` value may 400, but the
   failure path of real transcription must never return a non-empty error body.
2. `bazarr_detect_language_error_is_200` — when detection fails, `/bazarr/detect-language`
   returns **HTTP 200** with body
   `{"detected_language":"Unknown","language_code":"und"}` (never a 4xx/5xx), and
   a success returns `{"detected_language":<name>,"language_code":<code>}` with no
   extra keys.

Plus a `submate-bazarr` unit assertion that
`serde_json::to_value(LanguageDetectionResponse{..})` has exactly the keys
`detected_language` and `language_code`.
