# bug-parity: submate-paths leaks `./` for dot-relative video paths

**Crate:** `submate-paths`
**Symbol:** `build_subtitle_path` → `assemble_subtitle_path` → `join_parent` (`rust/crates/submate-paths/src/lib.rs`)
**Spec:** `submate/paths.py::build_subtitle_path` (`str(Path(video_path).parent / subtitle_name)`)

## Summary

When `video_path` starts with a `./` (current-dir) segment, the Rust port
keeps the leading `./` in the output, while Python's `pathlib` collapses it.
`str(Path("./movie.mp4").parent / name)` is `name` (parent is `.`, and joining
`Path(".") / name` drops the `.`), but camino's `Utf8Path::new("./movie.mp4").parent()`
is `Some(".")`, and `join_parent` only special-cases the **empty** parent, so a
`.` parent is emitted verbatim as `./<name>`.

This passes the current parity test only because `fixtures/paths/path_cases.json`
has no `./`-prefixed input — the fixture set under-covers this code path.

## Exact diff (Python golden vs Rust), `language="eng"`, default naming

Reproduced by feeding identical inputs to `submate.paths.build_subtitle_path`
(Python) and `submate_paths::build_subtitle_path` (Rust).

| input `video_path`   | Python golden        | Rust actual            | match |
|----------------------|----------------------|------------------------|-------|
| `./movie.mp4`        | `movie.eng.srt`      | `./movie.eng.srt`      | ✗     |
| `./a/movie.mp4`      | `a/movie.eng.srt`    | `./a/movie.eng.srt`    | ✗     |
| `././movie.mp4`      | `movie.eng.srt`      | `./movie.eng.srt`      | ✗     |

First differing character: position 0 — Python has no `./` prefix, Rust does.

All other probed edge cases already match (trailing dot, hidden dotfile,
`/media/movies/` trailing slash, no-extension, `.`-only, `../movie.mp4`,
`/a//movie.mp4` double-slash, extension without leading dot).

### Underlying camino vs pathlib parent semantics

```
input            pathlib parent   camino parent
"./movie.mp4"    "."              Some(".")
"./a/movie.mp4"  "a"              Some("./a")   <- camino keeps leading ./
"a/movie.mp4"    "a"              Some("a")
"/media/movies/" "/media"         Some("/media")
```

camino preserves the leading `./` (it does not normalize `.` components),
whereas `pathlib.PurePosixPath` drops a leading `.` directory component on
construction. So the fix must normalize a leading-`.`/`./` segment out of the
parent, not just guard the empty-parent case.

## Suggested fix (net-small, in `join_parent`)

`join_parent` currently:

```rust
fn join_parent(video_path: &str, name: &str) -> String {
    match Utf8Path::new(video_path).parent() {
        Some(parent) if !parent.as_str().is_empty() => format!("{parent}/{name}"),
        _ => name.to_string(),
    }
}
```

The parent string must have a leading `.` / `./` collapsed the way pathlib does
on construction (drop a sole `.`, strip a leading `./`). Minimal approach:
normalize the parent to its pathlib-equivalent string before joining, e.g. treat
parent `"."` as empty, and strip a leading `"./"` from the parent
(`"./a"` → `"a"`). Implementer should verify against the full probe table above,
not just the two-line cases — `././a/movie.mp4` etc. collapse the same way.

## Falsifier

`cargo test --manifest-path rust/Cargo.toml -p submate-paths --test parity`
passes with a `path_cases.json` that includes a `dot_relative`
(`video_path="./movie.mp4"` → `movie.eng.srt`) and `dot_relative_nested`
(`video_path="./a/movie.mp4"` → `a/movie.eng.srt`) case.

NOTE: fixtures are merge-denylisted for implementers (`rust/fixtures/README.md`)
and regenerated only via `rust/fixtures/capture/capture_paths.py`. The capture
script's `BUILD_CASES` list must gain these two cases (re-run capture to emit the
golden) **and** `src/lib.rs::join_parent` must be fixed, or the new golden will
fail. Both land together.

## Repro commands

```sh
# Python golden
nix develop --command python3 - <<'PY'
from submate.paths import build_subtitle_path
for p in ["./movie.mp4", "./a/movie.mp4", "././movie.mp4"]:
    print(p, "->", build_subtitle_path(p, "eng"))
PY
# Rust actual: call submate_paths::build_subtitle_path(p, Some("eng"), &SubtitleNaming::default())
```
