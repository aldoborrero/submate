# Server engine selection (whisper | parakeet)

## Context

submate has two transcription engines behind the `Transcriber` trait: `WhisperBackend`
(whisper.cpp, all languages) and `ParakeetBackend` (transcribe.cpp/Parakeet, European
languages only, word-level timing). `Dispatcher::transcribe_pcm_with(engine, model, pcm,
opts)` already runs either polymorphically (it takes an `Arc<dyn Transcriber>`).

Engine selection is wired **only into the CLI** (`submate transcribe --engine …`, a
CLI-local `Engine` enum). The **server** path — `POST /bazarr/asr` and
`/bazarr/detect-language`, which is what Bazarr uses — is hardwired to whisper
(`WhisperBazarrTranscriber` in submate-cli, via `Dispatcher::transcribe_pcm`). So Bazarr
cannot use Parakeet at all.

Bazarr talks to submate through a **patched fork of Bazarr's `whisperai` provider**
(`dockerfiles/bazarr/whisperai.py` in the homelab repo): a 691-line copy that hijacks the
registered `whisperai` slot to send the *target* subtitle language with `task=transcribe`
(submate then LLM-translates to it). It is fragile (re-diffed against upstream on every
Bazarr base bump) and carries vestigial whisper-only logic (`translate → English`) submate
never uses.

## Goals

1. **Server-side engine selection** so both engines are reachable via Bazarr: a config
   default plus a per-request override on `/bazarr/asr`.
2. A **dedicated Bazarr `submate` provider** that models submate's actual semantics
   (transcribe + LLM-translate to any target, per-engine) instead of forking `whisperai`.

## Status

- **Part A (submate server engine selection) — IMPLEMENTED** on branch
  `feat/server-engine-selection`. Commits: `Engine` moved to `submate-types`; a
  `server.engine` config default; a lenient `?engine=` query param on `/bazarr/asr`;
  and the engine-aware Bazarr transcriber (`transcriber_for` helper). Shipped behind
  the existing `model` / `parakeet` cargo features; whisper remains the default and
  behavior is unchanged for whisper requests.
- **Part B (Bazarr `submate` Python provider) — not yet implemented.** A separate plan
  follows.

## Non-goals

- Auto-fallback between engines (Parakeet fails → retry on whisper). The engine is explicit
  per request; fallback is a possible future extension.
- Making Parakeet handle CJK/Japanese — engine-inherent (Parakeet V3 is EU-only). Anime
  stays on whisper by configuring the engine per Bazarr instance.
- Upstreaming the Bazarr provider to Bazarr in this iteration (designed to be upstreamable;
  interim home is the homelab Docker image).

---

## A — submate server engine selection

### A1. Shared `Engine` type

Move `Engine` from the CLI-local `clap::ValueEnum` to **`submate-types`** so config, server,
and CLI share one type. `submate-types` is a **clap-free** crate by design, so `Engine`
**cannot** `#[derive(ValueEnum)]` there — mirror the existing `TranslationBackend` pattern:

- In `submate-types`: `Engine { Whisper, Parakeet }` with the full shared-enum derive set
  the parity harness requires — `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`
  plus strum `Display, EnumString, EnumIter`, `#[strum(serialize_all = "lowercase")]` /
  `#[serde(rename_all = "lowercase")]`, default `Whisper`.
- In `submate-cli`: a `parse_engine(&str) -> Result<Engine, …>` `value_parser` on the
  `--engine` arg (like `parse_backend` for `TranslationBackend`), **not** a `ValueEnum`
  derive.
- **`submate-types` parity** (`tests/parity.rs`): add an `Engine` block to
  `fixtures/types/enum_values.json`, a `check_enum("Engine", …)` call, and `"Engine"` to
  `COVERED_ENUMS` (the `no_uncovered_enums_in_golden` test asserts the golden covers exactly
  the listed enums). `check_enum` requires `Copy + Display + FromStr + Serialize +
  Deserialize + IntoEnumIterator + Debug` — hence `EnumIter` above.

### A2. Config

Add `engine: Engine` to `ServerSettings` (default `Engine::Whisper`), env
`SUBMATE__SERVER__ENGINE`. This is the server's default engine when a request does not
override it. Goldens to update (three files + one config-show row):

- `fixtures/config/defaults.resolved.json` — `"engine": "whisper"` under `server`
  (alphabetized).
- `fixtures/config/nested.resolved.json` — same, if it renders `server`.
- `fixtures/cli/config_show.defaults.rows.json` — a `["Server.Engine", "whisper"]` row in
  **field-declaration order** (after `Server.Concurrent Transcriptions`, before the
  `Translation.*` rows, per `preserve_order`).

### A3. HTTP layer (`submate-server`)

The Bazarr routes deliberately type their query params as `String`/`Option<String>` (not
typed enums) so a malformed value never trips axum's `422` query rejection — the routes
must "return an empty body, never an error envelope." Follow that pattern:

- `AsrParams` gains `engine: Option<String>` (serde `default`). Map it to `Engine` **by
  hand** — like `parse_output_format` maps `output` — with a small
  `parse_engine_param(Option<&str>) -> Option<Engine>` that lowercases and accepts
  `"whisper"`/`"parakeet"`; anything else (or absent) → `None` (⇒ use the config default).
  **Never** deserialize the enum directly in `Query<AsrParams>`.
- `BazarrTranscribeOpts` gains `engine: Option<Engine>`; the ASR handler passes the parsed
  value to the `BazarrTranscriber` seam.
- **Accepted spelling is lowercase `whisper` / `parakeet`, case-insensitively matched**;
  Part B must send exactly these. (Silent fallback to the default on a typo is intentional
  but means a casing bug is invisible — hence pinning it here.)

### A4. Transcriber (`submate-cli`)

`WhisperBazarrTranscriber` becomes engine-aware (`EngineBazarrTranscriber`):

- Holds `whisper_model: String`, `parakeet_model: String`, `default_engine: Engine`, the
  `Dispatcher`, decode/assemble opts, and the translate backend.
- **Model paths are used as-is** (both must be real file paths). The server has no
  `resolve_model` equivalent: `config.whisper.model` must already be a path here (the size-
  name shorthand only exists in the CLI's `resolve_model`), and `config.parakeet.model` is
  a raw path by design. `transcribe_pcm_with` returns `ModelNotFound` if the path is not a
  file → empty body (A5).
- Per `/bazarr/asr` request: `engine = opts.engine.unwrap_or(self.default_engine)`; pick the
  matching model; build the `Arc<dyn Transcriber>` for that engine (the Parakeet arm gated
  on the `parakeet` cargo feature via the existing `parakeet_transcriber()` shape — absent
  ⇒ a clean `Err`); run `Dispatcher::transcribe_pcm_with`.
- **`detect_language` stays on whisper regardless of `?engine=`.** Language detection needs
  broad coverage and Parakeet cannot detect the very languages (Japanese) that most need it;
  routing detection through Parakeet is useless. The `engine` param applies to `/bazarr/asr`
  only.
- `build_bazarr_transcriber` wires both model paths + `config.server.engine`.

### A5. Error handling

Every failure resolves to an **empty 200 body + a `tracing::warn`**, exactly like any other
transcription failure — no 5xx, no crash. Cases:

- Engine not compiled in → clean `Err` from `parakeet_transcriber()`.
- Selected engine's model path missing → `ModelNotFound`.
- **Unsupported language for Parakeet**: transcribe.cpp rejects a language its model does
  not support with an error (observed live: `status 10, "unsupported language"`), surfaced
  as a `TranscribeError` → empty body. This is *observed*, not a guaranteed language gate:
  submate does **not** pre-check the source language (which is auto-detected only during
  transcription). **The primary safeguard is operator config** — point Japanese/anime
  content at a `whisper`-engine provider (see B/Rollout). Do not advertise a runtime
  language gate that does not exist.

### A6. Tests

- `parse_engine_param`: absent/`whisper`/`parakeet`/mixed-case → expected; garbage → `None`.
- A route test: `?engine=parakeet` reaches the seam with the parakeet engine (fake
  `BazarrTranscriber` records `opts.engine`).
- Transcriber engine resolution: request param overrides the default; absent uses the
  default; `detect` ignores the param (always whisper).

---

## B — Bazarr `submate` provider

A first-class provider, not a `whisperai` fork:
`subliminal_patch/providers/submate.py` with `SubmateProvider(Provider)` and
`SubmateSubtitle(Subtitle)`, registered in Bazarr's provider list + settings schema.

### B1. Configuration

`endpoint`, `engine` (`whisper|parakeet`, default `whisper`), `timeout`, `response`,
`ffmpeg_path`, `pass_video_name`, `logger`.

- Sends `engine=<lowercase>` on the ASR request (A3's pinned spelling).
- **Advertised languages depend on `engine`:** with `whisper`, the broad Whisper set (~99);
  with `parakeet`, only Parakeet's supported (European) set — so Bazarr does not keep
  re-requesting a language the provider can never deliver. (Because A5 has no server-side
  language gate, this provider-side scoping is the real guardrail.)

### B2. Behaviour (submate-native, no whisper shoehorn)

- `list_subtitles(video, languages)`: one `SubmateSubtitle` per requested (advertised)
  language — submate transcribes the auto-detected source and LLM-translates to that target,
  so *every* requested language is a candidate directly. No `transcribe`/`translate→English`
  branching.
- `download_subtitle(sub)`: extract the audio track to WAV (s16le / mono / 16 kHz) honoring
  the audio delay and any forced stream, then
  `POST {endpoint}/asr?task=transcribe&language=<alpha2 target>&output=srt&encode=false&engine=<engine>`
  with `files={audio_file}` and `timeout=(response, timeout)`; set `sub.content` (empty ⇒
  no subtitle).
- Reuse the solid pieces of the current fork — ffmpeg audio extraction, ISO-639 mapping,
  audio-delay handling — dropping the whisper-only task logic.
- **Score:** a fixed provider score. The current fork's effective score is 66; keep 66 so
  existing `minimum_score` tuning (70 blocks it; ≤66 admits it — documented in configarr)
  stays valid, unless we deliberately revisit it.

### B3. Home

- **Upstream (eventual):** a Bazarr PR adding the provider + settings schema.
- **Interim (homelab):** the Docker image `COPY`s `submate.py` and registers it. If interim
  registration proves too invasive across Bazarr bumps, keep the `whisperai`-slot hijack for
  the homelab and ship the clean provider only upstream — decided during B's implementation.

---

## Rollout

1. **A** ships in a submate release; the native CT 2003 deploy is rebuilt from it, with both
   `SUBMATE__WHISPER__MODEL` and (if using Parakeet) `SUBMATE__PARAKEET__MODEL` set to real
   paths and `SUBMATE__SERVER__ENGINE` as the default.
2. **B** replaces `dockerfiles/bazarr/whisperai.py` (or is upstreamed to Bazarr).
3. `configarr.yml`: the provider config gains `engine`. `bazarr-anime` stays `whisper`;
   `bazarr` may use `parakeet` for European-language content.

## Resolved during review

- Query param is `Option<String>` mapped by hand (not a typed serde enum) to preserve the
  no-422 contract; accepted spelling pinned to lowercase `whisper`/`parakeet`.
- No server-side language gate exists; Parakeet-unsupported-language safety is
  best-effort (transcribe.cpp error → empty) plus provider-side advertised-language scoping
  and operator config.
- `Engine` lives in clap-free `submate-types`; the CLI parses it with a `value_parser`.
- `detect-language` stays on whisper.
