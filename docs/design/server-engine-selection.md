# Server engine selection (whisper | parakeet)

## Context

submate has two transcription engines behind the `Transcriber` trait: `WhisperBackend`
(whisper.cpp, all languages) and `ParakeetBackend` (transcribe.cpp/Parakeet, European
languages only, word-level timing). `Dispatcher::transcribe_pcm_with(engine, …)` already
runs either polymorphically.

Engine selection is wired **only into the CLI** (`submate transcribe --engine …`). The
**server** path — `POST /bazarr/asr` and `/bazarr/detect-language`, which is what Bazarr
uses — is hardwired to whisper (`WhisperBazarrTranscriber` in submate-cli). So Bazarr
cannot use Parakeet at all.

Bazarr talks to submate through a **patched fork of Bazarr's `whisperai` provider**
(`dockerfiles/bazarr/whisperai.py` in the homelab repo): a 691-line copy that hijacks the
registered `whisperai` slot to send the *target* subtitle language with `task=transcribe`
(submate then LLM-translates to it). It is fragile (must be re-diffed against upstream on
every Bazarr base bump) and carries vestigial whisper-only logic (`translate → English`)
that submate never uses.

## Goals

1. **Server-side engine selection** so both engines are reachable via Bazarr:
   a config default plus a per-request override.
2. A **dedicated Bazarr `submate` provider** that models submate's actual semantics
   (transcribe + LLM-translate to any target, per-engine) instead of forking `whisperai`.

## Non-goals

- Auto-fallback between engines (e.g. Parakeet fails → retry on whisper). The engine is
  explicit per request; fallback is a possible future extension.
- Making Parakeet handle CJK/Japanese — that is engine-inherent (Parakeet V3 is EU-only).
  Anime (Japanese) stays on whisper by configuring the engine per Bazarr instance.
- Upstreaming the Bazarr provider to Bazarr in this iteration. It is *designed* to be
  upstreamable, but the interim home is the homelab Docker image.

---

## A — submate server engine selection

### A1. Shared `Engine` type

Move `Engine` from a CLI-local `clap::ValueEnum` (`submate-cli`) to **`submate-types`** so
config, server, and CLI share one type. Derive serde (`rename_all = "lowercase"`) +
`FromStr`/`Display` (strum), matching the other shared enums. The CLI keeps using it as a
`ValueEnum`; the server parses it from a query string; config deserializes it.

### A2. Config

Add `engine: Engine` to `ServerSettings` (default `Engine::Whisper`), env
`SUBMATE__SERVER__ENGINE`. This is the server's default engine when a request does not
override it. Update the config parity + `config show` goldens.

### A3. HTTP layer (`submate-server`)

- `AsrParams` and `DetectParams` gain `engine: Option<Engine>`, parsed **leniently** from
  `?engine=` (an unknown value deserializes to `None`, never a 4xx — the routes must never
  reject, per the existing "empty body, never an error envelope" contract).
- `BazarrTranscribeOpts` gains `engine: Option<Engine>`; the ASR and detect handlers pass
  the parsed value through to the `BazarrTranscriber` seam.

### A4. Transcriber (`submate-cli`)

`WhisperBazarrTranscriber` becomes engine-aware (rename to `EngineBazarrTranscriber`):

- Holds `whisper_model: String`, `parakeet_model: String`, `default_engine: Engine`, the
  `Dispatcher`, decode/assemble opts, and the translate backend.
- Per request: `engine = opts.engine.unwrap_or(self.default_engine)`; pick the matching
  model (`whisper_model` / `parakeet_model`); build the `Arc<dyn Transcriber>` for that
  engine (the Parakeet arm gated on the `parakeet` cargo feature — absent ⇒ a clean
  `Err`); run `Dispatcher::transcribe_pcm_with`.
- `detect()` resolves the engine the same way.
- `build_bazarr_transcriber` wires both model paths + `config.server.engine`.

### A5. Error handling

Every failure — engine not compiled in, the selected engine's model unset/missing, an
unsupported language (Parakeet on Japanese ⇒ `Unsupported`) — resolves to an **empty 200
body + a `tracing::warn`**, exactly like any other transcription failure. No 5xx, no crash.

### A6. Tests

- `AsrParams` engine parsing: absent → `None`; `whisper`/`parakeet` → `Some`; garbage →
  `None` (lenient).
- A route test: `?engine=parakeet` reaches the transcriber with the parakeet engine (fake
  seam records the engine).
- Transcriber engine resolution: request param overrides the default; absent uses the
  default.

---

## B — Bazarr `submate` provider

A first-class provider, not a `whisperai` fork:
`subliminal_patch/providers/submate.py` with `SubmateProvider(Provider)` and
`SubmateSubtitle(Subtitle)`, registered in Bazarr's provider list + settings schema.

### B1. Configuration

`endpoint`, `engine` (`whisper|parakeet`, default `whisper`), `timeout`, `response`,
`ffmpeg_path`, `pass_video_name`, `logger`. Advertises the broad Whisper language set
(~99); when `engine=parakeet` and a non-EU language is requested, submate returns an empty
body and the provider yields no subtitle (no crash).

### B2. Behaviour (submate-native, no whisper shoehorn)

- `list_subtitles(video, languages)`: one `SubmateSubtitle` per requested language —
  submate transcribes the (auto-detected) source and LLM-translates to that target, so
  *every* requested language is a candidate directly. No `transcribe`/`translate→English`
  branching.
- `download_subtitle(sub)`: extract the audio track to WAV (s16le / mono / 16 kHz) honoring
  the audio delay and any forced stream, then
  `POST {endpoint}/asr?task=transcribe&language=<alpha2 target>&output=srt&encode=false&engine=<engine>`
  with `files={audio_file}` and `timeout=(response, timeout)`; set `sub.content` from the
  response body (empty ⇒ no subtitle).
- Reuse the solid, well-tested pieces of the current `whisperai.py` fork — ffmpeg audio
  extraction, the ISO-639 language mapping, audio-delay handling — but drop the
  whisper-only task logic.
- **Score:** a fixed provider score (documented) so operators can set `minimum_score`
  at/below it (the current fork scores 66; keep or revisit).

### B3. Home

- **Upstream (eventual):** a Bazarr PR adding the provider + its settings schema.
- **Interim (homelab):** the Docker image `COPY`s `submate.py` and registers it (provider
  list + settings). If interim registration proves too invasive across Bazarr bumps, keep
  the `whisperai`-slot hijack for the homelab and ship the clean provider only upstream —
  decided during implementation of B.

---

## Rollout

1. **A** ships in a submate release; the native CT 2003 deploy is rebuilt from it.
2. **B** replaces `dockerfiles/bazarr/whisperai.py` (or is upstreamed to Bazarr).
3. `configarr.yml`: the provider config gains `engine`. `bazarr-anime` stays `whisper`;
   `bazarr` may use `parakeet` for European-language content.

## Open questions

- B's interim home if registration is too invasive (clean provider upstream-only vs. keep
  the homelab hijack).
- The provider score value.
