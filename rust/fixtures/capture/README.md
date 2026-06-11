# Fixture capture

Scripts that emit the golden fixtures in `rust/fixtures/` from the **Python**
submate. Run once (and whenever the Python spec changes), then commit the
results. They are NOT part of the grind.

Run inside the nix devshell (provides the Python env + ffmpeg):

```sh
nix develop --command bash -c '
  export PYTHONPATH="$PWD:$PWD/rust/fixtures/capture"
  cd rust/fixtures/capture
  ./run_deterministic.sh        # no media needed
'
```

## Two groups

**Deterministic (no media)** — run automatically, fully portable:

| Script | Emits | Falsifier |
|---|---|---|
| `capture_enums.py` | `types/enum_values.json` | submate-types `parity::enum_values` |
| `capture_lang.py` | `lang/lang_conversions.json` | submate-lang `parity::lang_conversions` |
| `capture_paths.py` | `paths/path_cases.json` | submate-paths `parity::path_cases` |
| `capture_config.py` | `config/*.{env,resolved.json}` | submate-config `parity::{defaults,env_nesting,validators}` |

`capture_config.py` normalizes home-derived values (the queue `db_path`) to the
tokens `${XDG_DATA_HOME}` / `${HOME}`; the Rust config tests apply the same
substitution before comparing.

**Media-dependent** — pass a short (≤5 s) sample clip; these need a Whisper
model and/or ffmpeg, so they're run manually:

| Script | Emits | Notes |
|---|---|---|
| `capture_media.py <clip.mkv>` | `media/<clip>.probe.json`, `media/<clip>.pcm.sha256` | needs ffmpeg/ffprobe |
| `capture_stablets.py <clip.wav>` | `stablets/<clip>/{00_raw,01_regroup_*,02_suppress}.json`, `03.{srt,vtt}`, `audio.f32`, `stablets/regroup_parse.json` | needs a Whisper model; the crown-jewel staged goldens |
| `capture_translate.py` | `translate/*.{in.srt,out.srt}`, `translate/mock_llm.json`, `translate/chunking.json` | stubs the LLM deterministically (no network) |

Use tiny public-domain clips. Keep them out of git unless small; commit only the
emitted fixtures (audio.f32 for one ≤5 s clip is a few hundred KB — fine; use
git-LFS only if a clip itself must be committed).
