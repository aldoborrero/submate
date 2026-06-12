"""Capture STAGED stable-ts goldens for one clip — the highest-value fixtures.

Usage (inside the devshell; downloads a Whisper model on first run):
    python capture_stablets.py /path/to/clip.wav [--model tiny] [--clip-name clipA]

Falsifier targets:
  stable-ts parity::model_roundtrip   <- 00_raw.json
  stable-ts parity::regroup_parse     <- regroup_parse.json
  stable-ts parity::regroup_apply     <- 01_regroup_<op>.json
  stable-ts parity::wav2mask          <- audio.f32 (+ Rust-side mask)
  stable-ts parity::suppress          <- 02_suppress.json
  stable-ts parity::output            <- 03.srt / 03.vtt

Strategy: capture `00_raw` once (no regroup, no suppress), then test each stage
in ISOLATION by rebuilding a fresh WhisperResult from 00_raw and applying just
that stage — so the Rust port checks B (regroup) and C (suppress) against exact
inputs, independent of each other. `03.{srt,vtt}` come from the real end-to-end
run so the final output matches submate's actual pipeline.

The submate config this mirrors: regroup="cm_sl=84_sl=42++++++1",
suppress_silence=True, word_timestamps=True, min_word_duration=0.1.
"""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

import stable_whisper
from _common import FIXTURES, write_json, write_text

REGROUP = "cm_sl=84_sl=42++++++1"


def _dump_f32(rel: str, samples) -> None:
    path = FIXTURES / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    buf = struct.pack(f"<{len(samples)}f", *[float(x) for x in samples])
    path.write_bytes(buf)
    print(f"wrote {rel} ({len(samples)} f32)")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("audio")
    ap.add_argument("--model", default="tiny")
    ap.add_argument("--clip-name", default=None)
    args = ap.parse_args()

    clip = Path(args.audio)
    name = args.clip_name or clip.stem
    base = f"stablets/{name}"

    model = stable_whisper.load_faster_whisper(args.model, device="cpu", compute_type="int8")

    # --- 00_raw: word timestamps, no post-processing -----------------------
    raw = model.transcribe_stable(str(clip), regroup=False, suppress_silence=False, word_timestamps=True)
    raw_dict = raw.to_dict()
    write_json(f"{base}/00_raw.json", raw_dict)

    # --- regroup_parse: the parsed op list for REGROUP ---------------------
    # parse_regroup_algo returns (callable, kwargs, msg) per op; record name+kwargs.
    ops = raw.parse_regroup_algo(REGROUP, include_str=True)
    write_json(
        "stablets/regroup_parse.json",
        [{"method": op[0].__name__, "kwargs": op[1]} for op in ops],
    )

    # --- 01_regroup_<op>: apply each op in isolation from a fresh raw ------
    # Rebuild from raw_dict each time so stages don't compound.
    for i, op in enumerate(ops):
        fn, kwargs = op[0], op[1]
        fresh = stable_whisper.WhisperResult(raw_dict)
        bound = getattr(fresh, fn.__name__)
        bound(**kwargs)
        write_json(f"{base}/01_regroup_{i}_{fn.__name__}.json", fresh.to_dict())

    # --- 02_suppress: suppress_silence applied to a fresh raw -------------
    # Re-run the engine with suppress on but regroup off to get the suppressed
    # word timings (and to let stable-ts load the audio the same way it will at
    # runtime). audio.f32 below is what the non-VAD DSP consumes.
    suppressed = model.transcribe_stable(str(clip), regroup=False, suppress_silence=True, word_timestamps=True)
    write_json(f"{base}/02_suppress.json", suppressed.to_dict())

    # --- audio.f32: the mono/16k f32 array the DSP sees -------------------
    # Best-effort via stable-ts's own loader; verify the dtype/rate matches the
    # non-VAD path (16kHz mono f32) when wiring the Rust wav2mask test.
    try:
        import torch
        from stable_whisper.audio import load_audio  # type: ignore
        from stable_whisper.stabilization import nonvad  # type: ignore

        samples = load_audio(str(clip), sr=16000)
        _dump_f32(f"{base}/audio.f32", samples)

        # non-VAD DSP intermediates the suppress-dsp (wav2mask) port is falsified
        # against — the exact Python functions, not a reimplementation.
        audio_t = torch.as_tensor(samples, dtype=torch.float32).flatten()
        loudness = nonvad.audio2loudness(audio_t)
        if loudness is not None:
            _dump_f32(f"{base}/loudness.f32", loudness.tolist())
        mask = nonvad.wav2mask(audio_t, sr=16000)
        if mask is not None:
            # bool mask -> 0.0/1.0 f32 so the Rust test compares with assert_f32_close
            _dump_f32(f"{base}/mask.f32", mask.float().tolist())
        else:
            print("WARN: wav2mask returned None (no silence in clip) — mask.f32 not written")
    except Exception as e:  # noqa: BLE001 — capture-time best effort
        print(f"WARN: could not dump audio/DSP goldens ({e}); wire the loader manually")

    # --- 03.srt / 03.vtt: the REAL end-to-end output ---------------------
    final = model.transcribe_stable(str(clip), regroup=REGROUP, suppress_silence=True, word_timestamps=True)
    write_text(f"{base}/03.srt", final.to_srt_vtt(word_level=False, vtt=False))
    write_text(f"{base}/03.vtt", final.to_srt_vtt(word_level=False, vtt=True))

    # --- transcribe parity golden: final segments for the STRUCTURAL pipeline
    # comparison (parity::assert_segments_close — whisper.cpp != faster-whisper,
    # so the Rust port is checked on count/timing/text-similarity, not bytes).
    segs = [
        {"start": round(s.start, 3), "end": round(s.end, 3), "text": s.text}
        for s in final.segments
    ]
    write_json(f"transcribe/{name}.segments.json", segs)
    write_text(f"transcribe/{name}.expected.srt", final.to_srt_vtt(word_level=False, vtt=False))
    print(f"staged stable-ts goldens for {name} -> {FIXTURES / base}")


if __name__ == "__main__":
    main()
