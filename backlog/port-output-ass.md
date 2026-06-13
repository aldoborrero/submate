# port-output: ASS subtitle output (segment-level)

## what
Add ASS (Advanced SubStation Alpha) output — the fansub/styling subtitle format
— as a sibling to the existing `to_srt_vtt`. Port the segment-level path of
`stable_whisper.result.WhisperResult.to_ass` (`segment_level=True,
word_level=False`), the direct analogue of `to_srt_vtt(word_level=False)`: one
`Dialogue` line per segment, no per-word karaoke highlighting (that is a
separate future item).

The output is a complete ASS file:
```
[Script Info]
ScriptType: v4.00+
PlayResX: 384
PlayResY: 288
ScaledBorderAndShadow: yes

[V4+ Styles]
Format: Name, Fontname, Fontsize, ...
Style: Default,Arial,24,&H00ff00,&Hffffff,&H0,&H0,0,0,0,0,100,100,0,0,1,1,0,2,10,10,10,0

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:0.00,0:00:5.90,Default,,0,0,0,,The Low World. This is a test ...
```
Mirror stable_whisper exactly, including the **ASS timestamp format**
`H:MM:SS.cc` where seconds are NOT zero-padded but minutes are and centiseconds
are 2 digits (`0:00:0.00`, `0:00:5.90`, `0:00:10.36`) — add a `sec2ass` helper
next to `sec2srt`/`sec2vtt`. Embedded segment-text newlines become `\N` (not
present in this fixture, but handle it).

## where
- `rust/crates/stable-ts/src/output.rs` — `sec2ass`, the `[Script Info]`/`[V4+
  Styles]`/`[Events]` header constants, a `dialogue_line(layer, start, end, text)`
  helper, and `pub fn to_ass(result: &WhisperResult, word_level: bool) -> String`
  (segment-level; `word_level=true` may return `unimplemented!`/be deferred).
- `rust/crates/submate-whisper/src/lib.rs` — a `to_ass(&self) -> String` wrapper
  on the result type, mirroring the existing `to_srt_vtt` wrapper.
- Export `to_ass` from `stable-ts` `lib.rs`.

## why
ASS is the standard for anime fansubs and any styled/positioned subtitle. The
data is already there; only the serializer is missing. Submate currently emits
SRT/VTT only.

## falsifies
`cargo test -p stable-ts` green, including `parity::output_ass`: parse
`stablets/clipA/00_raw.json` into a `WhisperResult` and assert
`to_ass(&result, false)` is **byte-identical** to the golden
`rust/fixtures/stablets/clipA/output.ass` (captured via
`WhisperResult.to_ass(segment_level=True, word_level=False)` on the same
`00_raw` result). This pins the header, the `Default` style line, `sec2ass`, and
the `Dialogue` event layout against the real stable_whisper output.
