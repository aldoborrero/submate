# align: format_language_for_filename — pin `native`, `iso_639_2_t`, and the unparseable fallback against a Python golden

**relates-to:** submate-paths (`rust/crates/submate-paths/src/lib.rs`,
`rust/crates/submate-paths/tests/parity.rs`), `rust/fixtures/paths/path_cases.json`

## Contract

ENUM `.value` → OUTPUT — `LanguageNamingType` selects which of the
`LanguageCode` 5-tuple columns (`iso_639_1`, `iso_639_2_t`, `iso_639_2_b`,
`name_en`, `name_native`) lands in the subtitle filename suffix. The contract is
that each of the five naming-type variants drives the *exact* Python output for
that column, byte-for-byte (the suffix becomes part of an on-disk filename that
Bazarr/Jellyfin/Plex match against).

The Rust port already implements all five branches and the unparseable
fallback. This item makes three of them **falsifiable** — today they have **no
golden-backed proof**.

## Python SPEC

`submate/paths.py` — `format_language_for_filename` / `build_subtitle_path`:

```python
match naming_type:
    case LanguageNamingType.ISO_639_1:   return lang_code.to_iso_639_1() or ""
    case LanguageNamingType.ISO_639_2_T: return lang_code.to_iso_639_2_t() or ""
    case LanguageNamingType.ISO_639_2_B: return lang_code.to_iso_639_2_b() or ""
    case LanguageNamingType.NAME:        return lang_code.to_name(in_english=True) or ""
    case LanguageNamingType.NATIVE:      return lang_code.to_name(in_english=False) or ""
```

and the unparseable fallback (the `match` is never reached):

```python
if lang_code is LanguageCode.NONE:
    # Fall back to original string if we can't parse it
    return language if isinstance(language, str) else ""
```

`to_name(in_english=False)` returns `name_native` — the **non-ASCII** column
(`Deutsch`, `Español`, `Français`, `日本語`, `中文`, …). `to_iso_639_2_t` returns
the **terminological** code, which for ~20 languages diverges from the
default-`B` (bibliographic) code (`deu`≠`ger`, `zho`≠`chi`, `fra`≠`fre`).

## What `path_cases.json` covers today (the gap)

`rust/fixtures/paths/path_cases.json` `build_subtitle_path` cases exercise only:

- `naming_type: "iso_639_1"` (`eng` → `movie.en.srt`),
- `naming_type: "name"`     (`eng` → `movie.English.srt`),
- the **default** (`iso_639_2_b`, via `eng`→`eng`, `fra`→`fre`, `spa`→`spa`).

Never exercised by any golden:

1. **`native`** — the only path that emits non-ASCII bytes into a filename
   (`to_name(false)` → `name_native`). Most encoding/column-selection-prone
   branch; entirely unpinned.
2. **`iso_639_2_t`** — never present in the fixture at all. For T/B-divergent
   languages this differs from the default-B output, so a swap of the T/B
   columns (or pointing `Iso6392T` at `to_iso_639_2_b`) is invisible today.
3. **The unparseable fallback** — an input that `LanguageCode::from_string`
   resolves to `None` must return the *original input string* verbatim,
   regardless of naming type (the `match` is skipped). No golden exercises a
   non-resolving language.

The parity harness is **already wired** for these: `naming_type_from` in
`rust/crates/submate-paths/tests/parity.rs` already maps `"iso_639_2_t"` →
`Iso6392T` and `"native"` → `Native`. The only thing missing is fixture rows —
this is a pure append-only golden + capture change, no production code expected.

## Where

- `rust/fixtures/capture/` — the paths capture script (whatever generates
  `path_cases.json`): add `build_subtitle_path` cases for the naming types and
  the unparseable input below, captured from
  `submate.paths.build_subtitle_path`.
- `rust/fixtures/paths/path_cases.json` — regenerate with the new cases. Keep
  the JSON UTF-8 (the native column carries multibyte codepoints).
- `rust/crates/submate-paths/tests/parity.rs` (`path_cases`) — no change needed;
  it already iterates every `build_subtitle_path` case and parses these naming
  types. The new rows are picked up automatically.

## Golden values (verified against `submate/language.py` rows + `paths.py`)

| input language | naming_type   | expected `build_subtitle_path("movie.mp4", …)` |
|----------------|---------------|------------------------------------------------|
| `de` (German)  | `iso_639_2_t` | `movie.deu.srt`   |
| `de` (German)  | `iso_639_2_b` | `movie.ger.srt`   |
| `de` (German)  | `native`      | `movie.Deutsch.srt` |
| `zh` (Chinese) | `iso_639_2_t` | `movie.zho.srt`   |
| `zh` (Chinese) | `iso_639_2_b` | `movie.chi.srt`   |
| `zh` (Chinese) | `native`      | `movie.中文.srt`  |
| `ja` (Japanese)| `native`      | `movie.日本語.srt` |
| `es` (Spanish) | `native`      | `movie.Español.srt` |
| `fr` (French)  | `native`      | `movie.Français.srt` |
| `klingon`      | `native`      | `movie.klingon.srt` (NONE → original string, no naming) |
| `klingon`      | `iso_639_2_t` | `movie.klingon.srt` (NONE → original string, no naming) |

(`GERMAN = ("de","deu","ger","German","Deutsch")`,
`CHINESE = ("zh","zho","chi","Chinese","中文")`,
`JAPANESE = ("ja","jpn","jpn","Japanese","日本語")`,
`SPANISH = ("es","spa","spa","Spanish","Español")`,
`FRENCH = ("fr","fra","fre","French","Français")`.)

## Why

`language_naming_type` is a live config field (`SUBMATE__SUBTITLE__LANGUAGE_NAMING_TYPE`,
default `iso_639_2_b`) that selects the on-disk subtitle filename suffix. If a
user sets `native`, the Rust port must write `movie.日本語.srt` — exactly the
bytes Python writes — or downstream media-server subtitle matching (and the
skip-if-target-exists logic, which globs for that filename) breaks. The `native`
and `iso_639_2_t` branches and the NONE-fallback are real, reachable behaviors
that currently rest on implementation inspection alone, not a golden.

## Falsifies

After the rows land, `cargo test -p submate-paths path_cases` must hold for each
new case. Concrete regressions the new golden would catch (all green today):

- Point `LanguageNamingType::Native` at `to_name(true)` (English) instead of
  `to_name(false)` → `de`/`native` yields `movie.German.srt`, mismatching
  `movie.Deutsch.srt`. **Caught only with the native rows.**
- Swap the `Iso6392T`/`Iso6392B` arms (or both → `to_iso_639_2_b`) → `de`/
  `iso_639_2_t` yields `movie.ger.srt` instead of `movie.deu.srt`. **Caught only
  with a T-divergent row; the existing `name`/`iso1`/default cases stay green.**
- Drop the NONE fallback (return `""` for unparseable) → `klingon`/`native`
  yields `movie..srt` instead of `movie.klingon.srt`. **Caught only with a
  non-resolving input.**

Robust assertion: assert the produced path string equals the captured golden
byte-for-byte (UTF-8), including the multibyte native cases — do not normalize
or lowercase.
