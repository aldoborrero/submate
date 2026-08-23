# Design: Parakeet transcription via transcribe.cpp

Status: **proposed** · Scope: add a second transcription backend to submate so it
can run **Parakeet** (and other `transcribe.cpp` model families) alongside the
existing whisper.cpp engine.

## Summary

submate transcribes with **whisper.cpp** (via `whisper-rs`) today. This proposes
adding **[transcribe.cpp](https://github.com/handy-computer/transcribe.cpp)** as
a *second, feature-gated* backend, primarily to run **NVIDIA Parakeet** models,
which are typically faster and hallucinate less on music/silence than Whisper —
a real win for anime and other music-heavy media.

The transcription core already separates **raw ASR** from **post-processing**
(stable-ts regroup, silence suppression, VAD, `SRT/VTT/ASS` rendering) — those
operate on `WhisperResult`. A new backend only has to *produce a `WhisperResult`*;
everything downstream is reused unchanged. The Bazarr ASR provider stays
transparent to which engine ran.

## Why not just replace whisper.cpp?

Because `transcribe.cpp`'s **Whisper** path only surfaces **segment-level**
timestamps (confirmed by the binding's own `transcribe-file` example:
`max_timestamp_kind` is `Segment` for whisper). submate's current `whisper-rs`
backend already produces **word-level** timings, which stable-ts and word-level
SRT depend on. Routing Whisper through `transcribe.cpp` would therefore *regress*
word→segment.

**Decision:** keep `whisper-rs` as the Whisper backend; add `transcribe.cpp`
specifically for **Parakeet** (word/token timestamps) and its other families.
The two backends coexist behind a trait; config selects one.

## The gating question — timestamps — is answered

`transcribe.cpp` exposes per-row timestamps in **int64 milliseconds** at three
granularities, with a runtime capability query:

- `TimestampKind`: `None | Auto | Segment | Word | Token`
- `model.capabilities().max_timestamp_kind` — the finest a given model produces.
- Requesting finer than a model supports returns a clean
  `TRANSCRIBE_ERR_UNSUPPORTED_TIMESTAMPS`.

The Rust result type (`transcribe_cpp::Transcript`) already materializes owned
`segments`, `words`, `tokens` (text copied at the FFI boundary), plus detected
`language`, `timings`, and (bonus, unused for now) speaker diarization.

> **Resolved by the spike:** `handy-computer/parakeet-tdt-0.6b-v3` (Q5_K_M)
> reports `max_timestamp_kind == Token`, and requesting that max populates *both*
> `words` and `tokens` — transcribing `samples/jfk.wav` yields 1 segment, 22 words
> (first word `"And" [240 → 560 ms]`), 38 tokens. So `ParakeetBackend`, which folds
> `Transcript.words`, gets real word-level timing. (The earlier note that the
> example "only proved Whisper == Segment" was the pre-spike state.)

## API surface (Rust binding)

```rust
use transcribe_cpp::{Model, RunOptions, TimestampKind};

let model   = Model::load(&gguf_path)?;       // load a GGUF (Parakeet / Whisper / …)
let caps    = model.capabilities();           // arch, backend, max_timestamp_kind
let mut sess = model.session()?;              // one session per concurrent worker
let out = sess.run(&pcm_f32, &RunOptions {    // 16 kHz mono f32 PCM
    timestamps: caps.max_timestamp_kind,      // ask for the finest supported
    ..Default::default()
})?;                                          // -> Transcript
```

`Model` is loadable once and shared; `session()` is the per-runner handle — a
clean fit for submate's existing `Dispatcher` (one session per semaphore permit).

## Result mapping: `Transcript` → `WhisperResult`

The mapping is direct (times are ms → seconds):

| submate `WhisperResult` | transcribe.cpp `Transcript` |
|---|---|
| `language: String` | `language: Option<String>` (fall back to the forced hint) |
| `text: String` | `text` |
| `segments[].text / start / end` | `Segment.text` / `t0_ms` / `t1_ms` |
| `segments[].words[]` | slice `words[first_word .. first_word+n_words]` |
| `WhisperWord.word / start / end` | `Word.text` / `t0_ms` / `t1_ms` |
| `WhisperWord.probability` | mean of the word's tokens' `Token.p` (via `first_token`/`n_tokens`) |

`transcribe.cpp` gives strictly *more* than submate consumes (token confidences,
diarization) — nothing is missing.

## Proposed shape

```
crates/submate-whisper  →  gains a backend trait (name stays for now to avoid churn):

    #[async_trait] pub trait Transcriber {
        async fn transcribe_pcm(&self, pcm: Pcm, opts: TranscribeOptions)
            -> Result<WhisperResult, TranscribeError>;
        fn capabilities(&self) -> BackendCaps;   // max timestamp kind, supports translate-task, langs
    }

    impl Transcriber for WhisperBackend  { … }   // whisper-rs        [feature "whisper"]   (word-level)
    // new crate:
crates/submate-parakeet:
    impl Transcriber for ParakeetBackend { … }   // transcribe.cpp    [feature "parakeet"]  (word/token)

submate-config:   transcription.engine = "whisper" | "parakeet"  + model (GGUF) path
flake.nix:        submate-parakeet-{cpu,cuda,vulkan,metal}   (mirrors the whisper GPU variants)
submate-server / submate-bazarr:  unchanged — they call the trait; the ASR provider is engine-agnostic
```

The `Dispatcher`, stable-ts pipeline, VAD, and `to_srt_vtt/to_ass/…` renderers
are backend-agnostic and unchanged.

### Options that do and don't carry over

`TranscribeOptions` is Whisper-decoder-shaped. For Parakeet:

| Option | Whisper | Parakeet (CTC/TDT/RNN-T) |
|---|---|---|
| `language` | ✅ | ✅ (V3 multilingual; V2 English-only) |
| `task = Translate` (→English) | ✅ | likely **unsupported** → `UNSUPPORTED_TASK`; surface as a clear error |
| `initial_prompt` | ✅ | n/a |
| `beam_size`, `temperature`, `*_threshold` | ✅ (decoder knobs) | n/a — ignored |
| `max_len` | ✅ | maps to segment splitting if the family supports it |

Plan: keep the common fields (`language`, `task`) in the shared trait; treat the
Whisper-only knobs as backend-specific (ignored by Parakeet). `capabilities()`
advertises `supports_translate_task` and the language set so the server/CLI can
reject unsupported requests up front rather than mid-run.

## De-risking spike — **done, green**

Both prerequisites checked out:

1. **The C++ backend builds under our nix** — `transcribe-cpp-sys`'s cmake `build.rs`
   compiles the vendored C++/ggml with the devshell's existing cmake + clang (added
   for whisper.cpp), no flake changes needed.
2. **Parakeet yields word timestamps** — the `transcribe-file` example on
   `parakeet-tdt-0.6b-v3` (Q5_K_M, CPU) reports `max_timestamp_kind == Token` and
   populates 22 words / 38 tokens for `samples/jfk.wav`, first word `"And"
   [240 → 560 ms]`, transcript verbatim-correct.

So the plan stands as written — no token→word fallback needed, and Parakeet is not
stuck on a GPU build.

## Phased plan

1. **Spike** — confirm Parakeet timestamps + nix build (above).
2. **Trait** — introduce `Transcriber` in `submate-whisper`; make the current
   `whisper-rs` path an impl. No behavior change; pure refactor with the existing
   tests as the guard.
3. **Backend crate** — `submate-parakeet` wrapping `transcribe-cpp`; map
   `Transcript → WhisperResult`; feature-gate `parakeet`.
4. **Config + wiring** — `transcription.engine`; the CLI/server build the selected
   backend; `capabilities()` gates unsupported tasks/languages.
5. **Nix** — `submate-parakeet-{cuda,vulkan,metal}` build variants.
6. **Docs** — README + `docs/architecture.md` gain the engine-selection note.

## Risks

- **transcribe.cpp is young** — pin the binding to a specific commit; watch for
  API churn.
- **Parakeet timestamp granularity** — the spike gates this; token-only would
  mean we aggregate to words ourselves.
- **GPU build surface** — ggml backends (CUDA/Vulkan/Metal) must build under nix
  the way whisper.cpp already does; expect flake work, not code work.
- **Model management** — GGUF acquisition (HF) is a new fetch path vs the
  `ggml-*.bin` whisper models; document it.

## Out of scope

- Removing the CLI's LLM `translate` (kept — it's useful standalone; Bazarr's
  sub→sub path goes through Lingarr instead).
- Streaming/real-time, diarization, and the non-Parakeet families
  (Moonshine/Voxtral/etc.) — the backend makes them *possible* later, but they
  are not part of this change.
