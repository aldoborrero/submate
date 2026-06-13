# align: Bazarr ASR/detect-language query-constraint failures are HTTP 422, not 400

**relates-to:** port-server-bazarr-asr, align-bazarr-route-signatures

## Contract

ROUTE SIGNATURES — status code + error-body *shape* of the two Bazarr routes
when a query parameter violates its `Query(...)` constraint. This corrects a
wrong claim in the existing `align-bazarr-route-signatures.md` and pins a
detail no test/golden covers today (`bazarr_router()` in the Rust server is
still an empty placeholder — `rust/crates/submate-server/src/lib.rs`).

## Python SPEC

`submate/server/handlers/bazarr/router.py` declares the ASR query params with
FastAPI `Query(...)` *constraints*:

```python
task:   str        = Query(default="transcribe", pattern="^(transcribe|translate)$")
output: str        = Query(default="srt",        pattern="^(srt|vtt|txt|json)$")
```

and detect-language:

```python
detect_lang_length: int = Query(default=30, ge=1, le=300)
detect_lang_offset: int = Query(default=0,  ge=0)
```

FastAPI validates these constraints *before the handler body runs*. A request
with `?output=xml`, `?task=foo`, `?detect_lang_length=0`, or
`?detect_lang_offset=-1` never reaches the handler — FastAPI raises a
`RequestValidationError`, which its default exception handler turns into:

- **HTTP status `422` Unprocessable Entity** (NOT 400), and
- a body whose `detail` is a **JSON list** of error objects (NOT a string):

```json
{"detail": [{"type": "string_pattern_mismatch",
             "loc": ["query", "output"],
             "msg": "String should match pattern '^(srt|vtt|txt|json)$'",
             "input": "xml", "ctx": {"pattern": "^(srt|vtt|txt|json)$"}}]}
```

(For the `int` `ge/le` params the `type` is `greater_than_equal` /
`less_than_equal` and `ctx` carries the bound; the `loc`/`msg`/`url` strings are
Pydantic-v2/FastAPI-version-specific.)

The handler-internal `if output not in ("srt","vtt","txt","json"): raise
ValueError(...)` in `submate/server/handlers/bazarr/handlers.py` is **dead code
for the HTTP path**: because `output` (and `task`) already carry a `pattern`,
an out-of-set value is rejected at the framework layer with 422 and the
`ValueError`→`HTTPException(400)` branch in `router.py` is never reached for
those params. The 400 branch (`except ValueError → detail=str(e)`) only fires
for a `ValueError` raised *inside* transcription for some other reason.

## Existing-artifact drift

`backlog/align-bazarr-route-signatures.md` says, twice:

> Errors: `ValueError` → **400** `detail=str(e)` (e.g. invalid `output`)

and the falsifier `bazarr_asr_response_headers`:

> an invalid `output=xml` yields **400**

Both are wrong for the wire behavior. `output=xml` yields **422** with a
list-valued `detail`, never a 400 with a string `detail`. An implementer who
codes the Rust route to return 400 for a malformed `task`/`output`/`detect_lang_*`
query param diverges from what Bazarr's client observes against the Python SPEC.

## Where

`rust/crates/submate-server/src/lib.rs` — the bazarr ASR + detect-language
handlers wired by `port-server-bazarr-asr` must reject constraint violations
with **422** and a list-valued `detail`, not the `ServerError::BadRequest`
(400, string `detail`) envelope used for the `User-Agent`/`ValueError` paths.
The existing `ServerError::BadRequest` → `{"detail": "<str>"}` (400) mapping is
correct for the Jellyfin User-Agent rejection and any genuine in-handler
`ValueError`, but it is the *wrong* shape for query-constraint failures.

## Why

`task`/`output` accept exactly the enum-value sets (`transcribe|translate` =
`TranscriptionTask.value`; `srt|vtt|txt|json`). The contract is not just the
accepted set but the *rejection* behavior: status 422 + `detail` as a list.
This is the one place where FastAPI's automatic validation envelope (422,
`detail: [...]`) differs from submate's hand-rolled `HTTPException` envelope
(4xx/5xx, `detail: "<str>"`), and the difference is observable on the wire.

## Falsifies

A `cargo test -p submate-server` case added with the bazarr route wiring:

`bazarr_asr_bad_query_param_is_422` — POST `/bazarr/asr?output=xml` (valid
audio body) returns **HTTP 422**, and `body["detail"]` is a JSON **array**
(`body["detail"].is_array()`), never a 400 with a string `detail`. Likewise
`/bazarr/detect-language?detect_lang_length=0` returns 422 with a list
`detail`. A *valid* `output`/`task` still flows to transcription (no 422),
proving the gate is constraint-scoped, not blanket.

Robust assertion (avoid pinning the version-specific `msg`/`url`/`ctx`):

```rust
assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY); // 422, not 400
assert!(body["detail"].is_array(), "FastAPI validation detail is a list");
```
