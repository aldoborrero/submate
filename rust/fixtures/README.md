# Golden fixtures — the parity contract

These files are **golden truth** captured from the existing Python `submate`.
The Rust port is "done" for a given layer when its output matches the golden
here. The `parity` crate (`rust/crates/parity`) loads these and asserts.

**Fixtures are merge-denylisted for grind implementers.** They change only via
a deliberate capture run (`capture/`), never as a side effect of porting code —
otherwise a wrong port could "fix" the test by rewriting the truth.

## Diff modes (pick per layer)

| Layer | Mode | Helper |
|---|---|---|
| config resolution, language table, paths, subtitle detection, mocked-LLM translation, **stable-ts regroup (B)**, **stable-ts output (D)** | **exact** | `assert_json_eq` / `assert_str_eq` |
| **stable-ts suppress_silence (C)** — same `audio.f32` in, timings out | exact within `1e-6` | `assert_f32_close` |
| **full transcription** (whisper.cpp ≠ faster-whisper) | structural within tolerance | `assert_segments_close(SegTol{count:1,time_ms:200,text_ratio:0.9})` |

Everything except transcription itself is exact-diffable — that is what lets a
grind agent prove correctness without a human.

## Layout

```
fixtures/
  capture/                     # Python scripts that EMIT the goldens (run once; not in the grind)
  config/   <case>.env      ->  <case>.resolved.json     # Settings.model_dump(mode="json")
  lang/     lang_conversions.json                        # every LanguageCode <-> iso pair
  paths/    path_cases.json                              # build_subtitle_path inputs -> output
  subtitle/ <name>.{srt,vtt,ass} -> <name>.detected.json # + *.srt round-trip goldens
  translate/ <name>.in.srt + mock_llm.json -> <name>.out.srt
  stablets/ <clip>/00_raw.json        # WhisperResult after ingest (word ts from whisper)
            <clip>/01_regroup_<algo>.json   # after EACH parse_regroup_algo stage
            <clip>/02_suppress.json   # after suppress_silence
            <clip>/03.srt 03.vtt      # final formatted output
            <clip>/audio.f32          # raw little-endian f32 PCM the DSP (C) consumes
            <clip>/loudness.f32       # nonvad.audio2loudness(audio.f32) — DSP intermediate
            <clip>/mask.f32           # nonvad.wav2mask(audio.f32) — 0/1 silence mask (C falsifier)
            regroup_parse.json        # "cm_sl=84_sl=42++++++1" -> parsed op list
  transcribe/ <clip>.wav -> <clip>.expected.srt + <clip>.segments.json  # structural
  media/    <clip>.mkv -> <clip>.probe.json + <clip>.pcm.sha256
```

## The stable-ts staged goldens (highest value)

The stable-ts slice is the riskiest port, so it is tested **stage by stage**
against exact intermediate data rather than only end-to-end:

1. `00_raw.json` — the `WhisperResult` right after word timestamps arrive.
2. `01_regroup_<algo>.json` — one file per regroup op in `cm_sl=84_sl=42++++++1`,
   so `clamp_max` then each `split_by_length` is checked independently.
3. `02_suppress.json` — after `suppress_silence` (non-VAD), fed `audio.f32`.
4. `03.srt` / `03.vtt` — final `to_srt_vtt` output.

`audio.f32` is the exact array Python fed to `audio2loudness`, dumped raw, so
the Rust DSP consumes byte-identical input and only the algorithm is under test.

## Regenerating

Fixtures come from `capture/` run against the Python tree (see `capture/README.md`).
Re-running must be **idempotent** — identical bytes — or a fixture is
non-deterministic and needs a seed/stub (the LLM is stubbed via `mock_llm.json`
for exactly this reason).
