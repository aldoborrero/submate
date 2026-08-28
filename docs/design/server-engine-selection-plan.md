# Server Engine Selection Implementation Plan (submate — part A)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the submate ASR server (`/bazarr/asr`) pick the transcription engine
(whisper | parakeet) via a `SUBMATE__SERVER__ENGINE` default plus a per-request `?engine=`
override, preserving the "empty body, never an error envelope" contract.

**Architecture:** A shared `Engine` enum in the clap-free `submate-types` crate; a config
default in `ServerSettings`; a lenient query param mapped by hand in `submate-server`; and
an engine-aware Bazarr transcriber in `submate-cli` that routes through the existing
`Dispatcher::transcribe_pcm_with(Arc<dyn Transcriber>, …)`. `detect-language` stays on
whisper.

**Tech Stack:** Rust (edition 2024), strum, serde, axum, tokio. Spec:
`docs/design/server-engine-selection.md`.

**Scope:** Part A only (submate server). Part B (the Bazarr `submate` Python provider) is a
separate plan, written after A ships.

**Conventions:** run everything through the dev shell — `nix develop -c cargo …`. The
`parakeet` feature pulls a C++ Vulkan build; default/`model`-feature builds are the fast
path for most tasks.

---

### Task 1: Move `Engine` to `submate-types`

Today `Engine` is a CLI-local `#[derive(ValueEnum)] enum Engine { Whisper, Parakeet }` in
`crates/submate-cli/src/main.rs`. Move it to the shared, clap-free `submate-types` crate
(mirroring `TranslationBackend`), and switch the CLI arg to a `parse_engine` value_parser.

**Files:**
- Modify: `crates/submate-types/src/lib.rs` (add `Engine`)
- Modify: `crates/submate-types/tests/parity.rs` (cover `Engine`)
- Modify: `fixtures/types/enum_values.json` (golden)
- Modify: `crates/submate-cli/src/main.rs` (drop local enum + `ValueEnum`; add `parse_engine`)

- [ ] **Step 1: Add the `Engine` golden block + coverage (failing test)**

In `fixtures/types/enum_values.json`, add an `Engine` object — **SCREAMING_SNAKE variant
keys**, lowercase serialized values (match the other enums, e.g. `"TINY_EN": "tiny.en"`):

```json
"Engine": { "WHISPER": "whisper", "PARAKEET": "parakeet" }
```

In `crates/submate-types/tests/parity.rs`: add `"Engine"` to the `COVERED_ENUMS` list; add
`Engine` to the existing `use submate_types::{…};` import; and add a `check_enum` call in the
**pairs form** the real API takes — `check_enum(name, &[(SCREAMING_SNAKE_name, variant), …])`
(the golden is loaded *inside* `check_enum`; it is not passed a golden ref and not
turbofished):

```rust
check_enum(
    "Engine",
    &[("WHISPER", Engine::Whisper), ("PARAKEET", Engine::Parakeet)],
);
```

- [ ] **Step 2: Run the types tests to verify they fail**

Run: `nix develop -c cargo test -p submate-types`
Expected: FAIL — `Engine` is not defined in `submate-types` (compile error), or
`no_uncovered_enums_in_golden` mismatch.

- [ ] **Step 3: Add the `Engine` enum to `submate-types`**

In `crates/submate-types/src/lib.rs`, next to `TranslationBackend`, add (match its derive
set exactly — the parity `check_enum` bound requires `Copy + Display + FromStr + Serialize +
Deserialize + IntoEnumIterator + Debug`):

```rust
/// Which speech-to-text engine transcribes the audio. `whisper` (whisper.cpp) is the
/// default and handles every language; `parakeet` (transcribe.cpp) has word-level
/// timestamps but only European languages.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default,
    strum::Display, strum::EnumString, strum::EnumIter,
    serde::Serialize, serde::Deserialize,
)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    #[default]
    Whisper,
    Parakeet,
}
```

- [ ] **Step 4: Run the types tests to verify they pass**

Run: `nix develop -c cargo test -p submate-types`
Expected: PASS.

- [ ] **Step 5: Switch the CLI to the shared `Engine` + a `parse_engine` value_parser**

In `crates/submate-cli/src/main.rs`:
- Delete the local `enum Engine { … }` + its `#[derive(… ValueEnum)]`.
- Add a parser next to `parse_backend`:

```rust
/// Parse `--engine` into the shared [`submate_types::Engine`] (clap-free crate, so no
/// `ValueEnum` derive — mirror `parse_backend`).
fn parse_engine(s: &str) -> Result<submate_types::Engine, String> {
    s.parse::<submate_types::Engine>()
        .map_err(|_| format!("unknown engine '{s}' (expected: whisper, parakeet)"))
}
```

- Change the `--engine` arg attribute from `value_enum` to
  `#[arg(long, value_parser = parse_engine, default_value = "whisper")]` and its field type
  to `submate_types::Engine`.
- Replace every `Engine::Whisper`/`Engine::Parakeet` reference in `main.rs` with
  `submate_types::Engine::…` (or add `use submate_types::Engine;`).

- [ ] **Step 6: Verify the CLI compiles + its tests pass**

Run: `nix develop -c cargo test -p submate-cli --features model`
Expected: PASS. The existing `engine_flag_parses` and `ensure_engine_available_gates_parakeet`
tests reference `Engine::…` **unqualified**, so with `use submate_types::Engine;` in scope
they compile and pass **unchanged** — no test edit needed.

- [ ] **Step 7: Commit**

```bash
git add crates/submate-types crates/submate-cli fixtures/types/enum_values.json
git commit -m "refactor(types): move Engine to submate-types with a CLI value_parser"
```

---

### Task 2: `SUBMATE__SERVER__ENGINE` config default

**Files:**
- Modify: `crates/submate-config/src/lib.rs` (`ServerSettings.engine`)
- Modify: `fixtures/config/defaults.resolved.json`, `fixtures/config/nested.resolved.json`
- Modify: `fixtures/cli/config_show.defaults.rows.json`

- [ ] **Step 1: Add the goldens (failing test)**

- `fixtures/config/defaults.resolved.json`: in the `server` object add `"engine": "whisper"`
  (alphabetical position among the server keys).
- `fixtures/config/nested.resolved.json`: same, if it renders a `server` object.
- `fixtures/cli/config_show.defaults.rows.json`: add `["Server.Engine", "whisper"]` in
  field-declaration order — after `["Server.Concurrent Transcriptions", …]` and before the
  first `Translation.*` row (serde `preserve_order`).

- [ ] **Step 2: Run config + cli tests to verify they fail**

Run: `nix develop -c cargo test -p submate-config -p submate-cli`
Expected: FAIL — resolved-config / config_show goldens now expect a field the struct lacks.

- [ ] **Step 3: Add the field**

In `crates/submate-config/src/lib.rs`, `ServerSettings`, add after
`concurrent_transcriptions`:

```rust
    /// Default transcription engine for the ASR server (`/bazarr/asr`); a request may
    /// override it with `?engine=`. `parakeet` needs a build with the feature + a
    /// `parakeet.model`.
    pub engine: submate_types::Engine,
```

and in its `Default` impl: `engine: submate_types::Engine::Whisper,`.

- [ ] **Step 4: Run config + cli tests to verify they pass**

Run: `nix develop -c cargo test -p submate-config -p submate-cli`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/submate-config fixtures/config fixtures/cli
git commit -m "feat(config): add server.engine default (SUBMATE__SERVER__ENGINE)"
```

---

### Task 3: Thread the engine through `submate-server`

The Bazarr routes type query params as `String` to dodge axum's 422 rejection; map the
engine by hand (like `parse_output_format`), never as a typed serde enum.

**Files:**
- Modify: `crates/submate-server/src/lib.rs` (`AsrParams`, a mapper, `BazarrTranscribeOpts`,
  the ASR handler, tests)

- [ ] **Step 1: Write the failing parser test**

In the `#[cfg(feature = "bazarr")]` tests of `crates/submate-server/src/lib.rs`:

```rust
#[test]
fn parse_engine_param_is_lenient() {
    use submate_types::Engine;
    assert_eq!(parse_engine_param(None), None);
    assert_eq!(parse_engine_param(Some("whisper")), Some(Engine::Whisper));
    assert_eq!(parse_engine_param(Some("Parakeet")), Some(Engine::Parakeet));
    assert_eq!(parse_engine_param(Some("garbage")), None);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `nix develop -c cargo test -p submate-server parse_engine_param_is_lenient`
Expected: FAIL — `parse_engine_param` not defined.

- [ ] **Step 3: Implement the mapper + wire it in**

Add near `parse_output_format`:

```rust
/// Map the `?engine=` query value to an [`Engine`], case-insensitively. Unknown/absent →
/// `None` (the caller uses the server's configured default). Never rejects the request.
#[cfg(feature = "bazarr")]
fn parse_engine_param(engine: Option<&str>) -> Option<submate_types::Engine> {
    engine?.to_ascii_lowercase().parse().ok()
}
```

- Add `engine: Option<String>` (with `#[serde(default)]`) to `AsrParams`.
- Add `engine: Option<submate_types::Engine>` to `BazarrTranscribeOpts`.
- In the ASR handler, set the opts' engine from `parse_engine_param(params.engine.as_deref())`.
- Leave `DetectParams` and the detect handler untouched (detect stays whisper).

- [ ] **Step 4: Run server tests to verify they pass**

Run: `nix develop -c cargo test -p submate-server`
Expected: PASS (parser test + the existing route tests still green — `BazarrTranscribeOpts`
gained an `Option` field the fakes can default).

- [ ] **Step 5: Commit**

```bash
git add crates/submate-server
git commit -m "feat(server): accept ?engine= on /bazarr/asr (lenient, default via config)"
```

---

### Task 4: Engine-aware Bazarr transcriber (`submate-cli`)

Make the production seam route per-request. Reuse the existing feature-gated
`parakeet_transcriber()` shape already in `main.rs`.

**Files:**
- Modify: `crates/submate-cli/src/main.rs` (`WhisperBazarrTranscriber` → `EngineBazarrTranscriber`,
  `build_bazarr_transcriber`, a route test)

- [ ] **Step 1: Write the failing engine-resolution test**

In the `main.rs` tests, add a unit test for the resolution helper (extract a small pure fn
`resolve_engine(opt: Option<Engine>, default: Engine) -> Engine` = `opt.unwrap_or(default)`
if one doesn't already fall out naturally):

```rust
#[test]
fn bazarr_engine_resolution_prefers_request() {
    use submate_types::Engine;
    assert_eq!(resolve_engine(Some(Engine::Parakeet), Engine::Whisper), Engine::Parakeet);
    assert_eq!(resolve_engine(None, Engine::Whisper), Engine::Whisper);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `nix develop -c cargo test -p submate-cli --features model bazarr_engine_resolution`
Expected: FAIL — `resolve_engine` not defined.

- [ ] **Step 3: Implement the engine-aware transcriber**

In `crates/submate-cli/src/main.rs`:
- Add `fn resolve_engine(opt: Option<submate_types::Engine>, default: submate_types::Engine)
  -> submate_types::Engine { opt.unwrap_or(default) }`.
- Rename `WhisperBazarrTranscriber` → `EngineBazarrTranscriber`; add fields
  `parakeet_model: String` and `default_engine: submate_types::Engine` (keep the existing
  `dispatcher`, `model_path` → rename to `whisper_model`, `backend`, `chunk_size`, `decode`,
  `assemble`).
- In its `transcribe(opts, pcm)`: `let engine = resolve_engine(opts.engine,
  self.default_engine);` then select `(model, transcriber)` as `(String, Arc<dyn
  submate_whisper::Transcriber>)`:
  - `Engine::Whisper`:
    ```rust
    // transcribe_pcm_with does NOT install the whisper.cpp log hook (transcribe_pcm did).
    submate_whisper::install_whisper_logging();
    (self.whisper_model.clone(),
     Arc::new(submate_whisper::WhisperBackend) as Arc<dyn submate_whisper::Transcriber>)
    ```
  - `Engine::Parakeet`:
    ```rust
    (self.parakeet_model.clone(), parakeet_transcriber().map_err(|e| e.to_string())?)
    ```
    `parakeet_transcriber()` returns `anyhow::Result<Arc<dyn Transcriber>>`; the seam returns
    `Result<_, String>`, so map the error. Without the `parakeet` feature its `#[cfg]` variant
    `Err`s cleanly → empty body.
  Then `self.dispatcher.transcribe_pcm_with(transcriber, model, pcm, options).await
  .map_err(|e| e.to_string())?`. Both model paths are used as-is (real files; missing →
  `ModelNotFound` → empty body).
- `detect()` stays whisper but **must** use the renamed field: change `self.model_path` →
  `self.whisper_model` (it keeps calling `transcribe_pcm`). It is not otherwise changed.
- `build_bazarr_transcriber`: set `whisper_model: config.whisper.model.clone()`,
  `parakeet_model: config.parakeet.model.clone()`, `default_engine: config.server.engine`.

Note: `parakeet_transcriber()` returns `anyhow::Result<Arc<dyn Transcriber>>`; adapt to the
seam's `Result<_, String>` with `.map_err(|e| e.to_string())?`.

- [ ] **Step 4: Run the CLI tests (model feature) to verify they pass**

Run: `nix develop -c cargo test -p submate-cli --features model`
Expected: PASS.

- [ ] **Step 5: Verify the parakeet feature still builds the server path**

Run: `nix develop -c cargo clippy -p submate-cli --features parakeet --all-targets -- -D warnings`
Expected: clean (the parakeet arm compiles the real backend).

- [ ] **Step 6: Commit**

```bash
git add crates/submate-cli
git commit -m "feat(cli): engine-aware Bazarr transcriber (server engine selection)"
```

---

### Task 5: End-to-end verification + docs

- [ ] **Step 1: Full workspace gate**

Run: `nix develop -c cargo test --workspace` then
`nix develop -c cargo clippy -p submate-cli -p submate-whisper --features model --all-targets -- -D warnings`
Expected: all green. Note: `--workspace` uses **default features** (no `model`), so it does
NOT compile the engine-aware transcriber — the `--features model` test + the `--features
parakeet` clippy (Task 4 Steps 4–5) are the real gate for Task 4's code.

- [ ] **Step 2: Manual smoke (default engine unchanged)**

Build `.#submate-vulkan-parakeet` (or run the server with `model`), POST a jfk PCM to
`/bazarr/asr` with no `?engine=` → whisper SRT; with `?engine=parakeet` + a parakeet model
configured → parakeet SRT; with `?engine=parakeet` and no parakeet model → empty body +
a warn in the log. (Document the exact commands used in the commit message.)

- [ ] **Step 3: Update `docs/design/server-engine-selection.md`** — mark part A implemented;
  note part B (Bazarr provider) is the next plan.

- [ ] **Step 4: Commit + open PR**

```bash
git add docs/design/server-engine-selection.md
git commit -m "docs: mark server engine selection (part A) implemented"
```
Open a PR for `feat/server-engine-selection` (needs the FIDO key — hand to Aldo).

---

## Notes for the executor

- **Never** turn a Bazarr query param into a typed serde enum — it reintroduces a 422 bug.
- The Parakeet arm is gated on the `parakeet` cargo feature; a build without it must still
  compile (the `#[cfg(all(feature="model", not(feature="parakeet")))]` `parakeet_transcriber()`
  returns a clean `Err`).
- All transcription failures render an **empty 200 body + `tracing::warn`** — do not add 5xx.
- Model config values are used as **real file paths** on the server (no `resolve_model`).
