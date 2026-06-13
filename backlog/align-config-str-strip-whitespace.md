# align: WhisperSettings/JellyfinSettings string fields strip surrounding whitespace; Rust config does not

**relates-to:** submate-config (`rust/crates/submate-config/src/lib.rs`)

## Contract

CONFIG KEYS / VALUES — the *resolved value* of the scalar `str` fields on two
settings models. Two Pydantic models carry
`model_config = SettingsConfigDict(str_strip_whitespace=True)`, which strips
leading/trailing whitespace from **every** `str` field they declare, at **every**
input layer (env var, JSON config-file, direct init). The Rust port has no
whitespace stripping for any scalar string field, so a value with surrounding
whitespace resolves to a *different string* than Python emits. No fixture
covers this today (`validators.env` exercises pipe-lists / JSON-kwargs / regroup
only).

## Python SPEC

`submate/config.py`:

```python
class WhisperSettings(BaseModel):
    model_config = SettingsConfigDict(str_strip_whitespace=True)
    model: str = Field(default=WhisperModel.MEDIUM, ...)
    compute_type: str = Field(default="int8", ...)
    # device/implementation are enums; transcribe_kwargs/folders are non-str
    ...

class JellyfinSettings(BaseModel):
    model_config = SettingsConfigDict(str_strip_whitespace=True)
    server_url: str = Field(default="", ...)
    api_key: str = Field(default="", ...)
    # libraries is list[str]
    ...
```

`str_strip_whitespace=True` makes Pydantic apply `str.strip()` to the value of
every `str`-typed field on the model before it is stored. It strips spaces,
tabs, and newlines; a whitespace-only value becomes the empty string. It is a
*model-level* config: only these two models have it. The other settings models
(`ServerSettings`, `TranslationSettings`, `QueueSettings`, `SubtitleSettings`,
`StableTsSettings`, `PathMappingSettings`) do **not** carry it, so their string
fields keep surrounding whitespace verbatim.

Empirically verified against `submate.config` (pydantic 2.11.7):

| env var (value, `‹·›` marks bounds)                    | Python resolves to        | Rust resolves to (current)   |
|--------------------------------------------------------|---------------------------|------------------------------|
| `SUBMATE__WHISPER__COMPUTE_TYPE` = `‹  float16  ›`     | `"float16"`               | `"  float16  "`  ← DIVERGES  |
| `SUBMATE__WHISPER__MODEL` = `‹  large-v3  ›`           | `"large-v3"`              | `"  large-v3  "` ← DIVERGES  |
| `SUBMATE__JELLYFIN__SERVER_URL` = `‹  http://j:8096 ›` | `"http://j:8096"`         | `"  http://j:8096 "` ← DIV.  |
| `SUBMATE__JELLYFIN__API_KEY` = `‹     ›` (5 spaces)    | `""` (empty)              | `"     "`        ← DIVERGES  |
| `SUBMATE__SERVER__ADDRESS` = `‹  1.2.3.4  ›`           | `"  1.2.3.4  "` (NO strip)| `"  1.2.3.4  "`  ← matches   |
| `SUBMATE__TRANSLATION__OLLAMA_MODEL` = `‹  llama3.2 ›` | `"  llama3.2 "` (NO strip)| `"  llama3.2 "`  ← matches   |

Note the asymmetry: `ServerSettings.address` and `TranslationSettings.ollama_model`
keep the whitespace in *both* Python and Rust (they match by accident, since
neither side strips). Only the four fields on the two stripping models diverge.

Affected scalar `str` fields (exhaustive): `whisper.model`,
`whisper.compute_type`, `jellyfin.server_url`, `jellyfin.api_key`.

Not affected by this item: list fields (`whisper.folders`, `jellyfin.libraries`)
already trim per-element via their own `parse_pipe_separated_*` `mode="before"`
validators — and the Rust `deserialize_pipe_list` already does `str::trim()` per
element — so list elements match regardless. Enum fields (`whisper.device`,
`whisper.implementation`) are out of scope: Python does *not* pre-strip before
enum coercion, so `SUBMATE__WHISPER__DEVICE="  cuda  "` raises a validation
error in Python rather than resolving to `cuda` (see "Adjacent" below).

## Where (Rust)

`rust/crates/submate-config/src/lib.rs`. `WhisperSettings.model`,
`WhisperSettings.compute_type`, `JellyfinSettings.server_url`, and
`JellyfinSettings.api_key` are plain `String` with `#[serde(default ...)]` and
no trimming `deserialize_with`. They pass the raw figment env/file string
through unchanged.

Fix shape (one option): add a `deserialize_with = "deserialize_stripped"` to
those four fields that trims like the existing `deserialize_pipe_list` does
per-element, e.g.

```rust
/// Ports Pydantic `str_strip_whitespace=True` (WhisperSettings/JellyfinSettings):
/// trim surrounding ASCII+Unicode whitespace from the resolved string at any
/// layer. A whitespace-only value becomes "".
fn deserialize_stripped<'de, D>(de: D) -> Result<String, D::Error>
where D: Deserializer<'de> {
    Ok(String::deserialize(de)?.trim().to_string())
}
```

applied as `#[serde(default = "...", deserialize_with = "deserialize_stripped")]`
on exactly those four fields — and *not* on the non-stripping models'
string fields (`server.address`, `translation.*_model`, `translation.*_url`,
`translation.*_api_key`, `queue.db_path`, `subtitle.force_detected_language_to`,
`subtitle.skip_if_internal_subtitle_language`, `path_mapping.*`), which Python
leaves untrimmed.

## Why

These values cross live boundaries: `whisper.model` selects the model file,
`whisper.compute_type` is passed to the backend, `jellyfin.server_url`/`api_key`
are concatenated into HTTP requests. A trailing space in a model name or a URL
changes behavior (wrong file lookup, malformed URL) and produces a
config-resolution string that differs from the Python golden byte-for-byte —
violating the "config resolution must match EXACTLY" contract for the pure-data
layer.

## Adjacent (do NOT fold in; separate falsifier if pursued)

`SUBMATE__WHISPER__DEVICE="  cuda  "` raises a `ValidationError` in Python
(whitespace is not stripped before the `Device` enum coerces, so `"  cuda  "`
is not a member). The Rust `Device` deserialization should likewise reject a
whitespace-padded enum value rather than silently trimming it. This is a
*rejection*-behavior contract distinct from the scalar-string trimming above;
file separately with its own falsifier if it matters.

## Falsifies

Add to `rust/crates/submate-config/tests/parity.rs` a case
`strip_whitespace_on_whisper_jellyfin` driven by a new fixture pair, e.g.
`rust/fixtures/config/strip.env`:

```
SUBMATE__WHISPER__MODEL=  large-v3
SUBMATE__WHISPER__COMPUTE_TYPE=  float16
SUBMATE__JELLYFIN__SERVER_URL=  http://jelly:8096
SUBMATE__JELLYFIN__API_KEY=
SUBMATE__SERVER__ADDRESS=  1.2.3.4
SUBMATE__TRANSLATION__OLLAMA_MODEL=  llama3.2
```

with `strip.resolved.json` captured from
`python -c "from submate.config import Config; import json,sys; print(json.dumps(Config().model_dump(mode='json'), sort_keys=True))"`
run under those env vars. Expected resolved values (the load-bearing diffs):

```
whisper.model              == "large-v3"          (trimmed)
whisper.compute_type       == "float16"           (trimmed)
jellyfin.server_url        == "http://jelly:8096"  (trimmed)
jellyfin.api_key           == ""                   (whitespace-only -> empty)
server.address             == "  1.2.3.4  "        (NOT trimmed)
translation.ollama_model   == "  llama3.2"         (NOT trimmed)
```

Today `Config::from_env(strip.env)` leaves `whisper.model` = `"  large-v3"`,
`whisper.compute_type` = `"  float16"`, `jellyfin.api_key` = `""` only if the
env layer drops empty — verify the empty/whitespace-only case explicitly — so
the diff against `strip.resolved.json` fails until the four fields trim.

Robust assertion (independent of figment env-trimming quirks): assert the four
stripping-model fields equal their trimmed forms AND that `server.address` /
`translation.ollama_model` retain their leading whitespace, proving the strip is
scoped to exactly the two `str_strip_whitespace=True` models, not blanket.

### Caveat for the fixture/test author (load-bearing)

The existing parity harness's `parse_env` (`tests/parity.rs`) does
`raw.lines().map(str::trim)` — it trims each **whole line** before
`split_once('=')`. That means a fixture line `SUBMATE__WHISPER__MODEL=  x  `
loses its *trailing* value whitespace (and any leading whitespace before the
key), so a `.env` fixture cannot faithfully carry trailing-whitespace test
values through this loader. Two consequences:

1. Do NOT reuse `parse_env` for this case as-is. Either set the env vars
   directly in the `figment::Jail` with literal whitespace
   (`jail.set_env("SUBMATE__WHISPER__MODEL", "  large-v3  ")`), or add a
   non-trimming loader for this fixture. Leading whitespace survives `parse_env`
   (the value side after `=` is untouched); only trailing whitespace is eaten.
   Test with both leading AND trailing whitespace to exercise full
   `str.strip()` parity.

2. Independently verify whether figment's `Env::prefixed(...).split("__")`
   provider trims env values before serde sees them. If figment itself trims,
   the *env* layer would match Python by coincidence and the divergence would
   only manifest via the JSON config-file layer (`Json::file`, which Python's
   `get_config(config_file)` path strips and Rust does not). Capture the golden
   via `os.environ[...] = "  large-v3  "` set inside Python (not a shell `export`,
   which some shells normalize), then run
   `Config().model_dump(mode='json')`. The four stripping-model fields must show
   trimmed values; `server.address`/`translation.ollama_model` must retain the
   whitespace — that asymmetry is the real signal regardless of which layer trims.
