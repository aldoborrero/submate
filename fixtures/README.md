# Golden fixtures — the characterization baseline

These files are the **golden baseline** for the workspace's `parity::*` tests:
each layer's output is asserted to match the golden here. The `parity` crate
(`crates/parity`) loads them and provides the assert helpers.

They were originally captured from the Python implementation that submate was
ported from; that implementation has since been removed, so the fixtures are now
the workspace's **own frozen snapshots**. Treat them as golden truth — change
them deliberately (when behavior intentionally changes), never as a side effect.

## Diff modes (pick per layer)

| Layer | Mode | Helper |
|---|---|---|
| config resolution, language table, paths, subtitle detection, mocked-LLM translation, **stable-ts regroup**, **stable-ts output** | **exact** | `assert_json_eq` / `assert_str_eq` |
| **stable-ts suppress_silence** — same `audio.f32` in, timings out | exact within `1e-6` | `assert_f32_close` |
| **full transcription** (model-dependent) | structural within tolerance | `assert_segments_close(SegTol{count:1,time_ms:200,text_ratio:0.9})` |

Everything except transcription itself is exact-diffable.

## Layout

```
fixtures/
  config/   <case>.env      ->  <case>.resolved.json     # resolved Config as JSON
  lang/     lang_conversions.json                        # every LanguageCode <-> iso pair
  subtitle/ basic.{srt,vtt}, single.srt, tagged_sample.ass # cue parse/round-trip goldens
  bazarr/   pcm/sine440.pcm                              # raw s16le PCM (f32 decode golden)
  translate/ <name>.in.srt + mock_llm.json -> <name>.out.srt
  stablets/ <clip>/00_raw.json        # WhisperResult after ingest (word ts from whisper)
            <clip>/01_regroup_<algo>.json   # after EACH parse_regroup_algo stage
            <clip>/02_suppress.json   # after suppress_silence
            <clip>/03.srt 03.vtt      # final formatted output
            <clip>/audio.f32          # raw little-endian f32 PCM the DSP consumes
            <clip>/loudness.f32       # audio2loudness(audio.f32) — DSP intermediate
            <clip>/mask.f32           # wav2mask(audio.f32) — 0/1 silence mask
            regroup_parse.json        # "cm_sl=84_sl=42++++++1" -> parsed op list
  transcribe/ <clip>.wav -> <clip>.expected.srt + <clip>.segments.json  # structural
  media/    <clip>.mkv -> <clip>.probe.json + <clip>.pcm.sha256
```

## The stable-ts staged goldens

The stable-ts slice is the trickiest part, so it is tested **stage by stage**
against exact intermediate data rather than only end-to-end:

1. `00_raw.json` — the `WhisperResult` right after word timestamps arrive.
2. `01_regroup_<algo>.json` — one file per regroup op in `cm_sl=84_sl=42++++++1`,
   so `clamp_max` then each `split_by_length` is checked independently.
3. `02_suppress.json` — after `suppress_silence` (non-VAD), fed `audio.f32`.
4. `03.srt` / `03.vtt` — final `to_srt_vtt` output.

`audio.f32` is the exact array fed to `audio2loudness`, dumped raw, so the DSP
consumes byte-identical input and only the algorithm is under test.

## Note

Because the original capture source is gone, these fixtures are **frozen**: they
are not regenerated. They remain valid as the characterization baseline for the
ported pure-data layers; update a golden by hand only when the corresponding
behavior changes on purpose.
