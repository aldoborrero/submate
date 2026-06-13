# align: Jellyfin webhook missing-field body failures are HTTP 422 with a list `detail`

**relates-to:** port-server-jellyfin-webhook, align-bazarr-asr-query-validation-422

## Contract

ROUTE SIGNATURES — status code + error-body *shape* of `POST /webhooks/jellyfin`
when the JSON body fails validation (a required field absent). This is the
body-validation analogue of `align-bazarr-asr-query-validation-422.md` (which
pins the *query*-constraint case for the Bazarr routes); the Jellyfin route has
no query/path constraints but a `Body(...)`-validated Pydantic model, and that
validation envelope is not pinned by any test or golden today.

## Python SPEC

`submate/server/handlers/jellyfin/router.py` declares the body param:

```python
@router.post("/jellyfin")
async def jellyfin_webhook(
    user_agent: str | None = Header(default=None),
    payload: JellyfinWebhookPayload = Body(...),
) -> dict:
    if not user_agent or "Jellyfin-Server" not in user_agent:
        raise HTTPException(status_code=400, detail="Invalid request - not from Jellyfin server")
    ...
```

`submate/server/handlers/jellyfin/models.py` makes two fields **required** (no
default):

```python
notification_type: str = Field(alias="NotificationType")
item_id:           str = Field(alias="ItemId")
# item_type / name / server_id default to None
model_config = {"populate_by_name": True}
```

FastAPI validates `payload` (and every other declared param) *before the
handler body runs*. A POST whose JSON omits `NotificationType` or `ItemId`
(or sends a non-string for them) never reaches the handler — FastAPI raises
`RequestValidationError`, whose default handler returns:

- **HTTP status `422` Unprocessable Entity** (NOT 400, NOT 500), and
- a body whose `detail` is a **JSON list** of error objects (NOT a string):

```json
{"detail": [{"type": "missing",
             "loc": ["body", "ItemId"],
             "msg": "Field required",
             "input": {"NotificationType": "ItemAdded"}}]}
```

(The `loc`/`msg`/`url`/`input` strings are Pydantic-v2/FastAPI-version-specific;
do not pin them.)

### Validation runs before the User-Agent gate

Because FastAPI validates the body *before* the handler body executes, the
`user_agent` → `HTTPException(400)` check **never fires for an invalid body**:
a request with a bad/missing User-Agent *and* a body missing `ItemId` returns
**422** (body validation), not 400 (UA gate). The 400 UA path is reachable only
when the body is structurally valid. An implementer who validates UA first and
returns 400 for the bad-UA-and-bad-body case diverges from the Python wire
behavior.

## Existing Rust artifact

`rust/crates/submate-jellyfin/src/lib.rs` — `JellyfinWebhookPayload` derives
`serde::Deserialize` with `notification_type: String` / `item_id: String`
required (no `#[serde(default)]`), aliased to `NotificationType` / `ItemId`
(plus snake-case aliases). `rust/crates/submate-server/src/lib.rs` mounts
`POST /webhooks/jellyfin` and extracts `Json(payload): Json<JellyfinWebhookPayload>`.

Two divergences from the Python wire shape:

1. **Body shape.** Axum's default `Json` rejection produces a **plain-text**
   body for the 422 (e.g. `Failed to deserialize the JSON body ...`), not
   FastAPI's `{"detail": [ ... ]}` JSON envelope. The Bazarr 422 align already
   establishes that constraint failures must carry a **list-valued `detail`**;
   the same applies here. The route must convert the extractor rejection into
   the FastAPI-shaped `{"detail": [<error-object>]}` body.

2. **Status confirmation.** Axum's `Json` extractor returns 422 for a *valid
   JSON document with the wrong shape* (missing field) but **400** for a
   *syntactically invalid JSON* (parse error). FastAPI returns **422** for both
   (a malformed JSON body is also a `RequestValidationError`). The Rust route
   must map *both* the parse-error and the missing-field cases to **422**, not
   let the syntactic-parse case leak through as 400.

The existing `ServerError::BadRequest` → `{"detail": "<str>"}` (400) mapping is
correct only for the UA rejection (structurally valid body, bad/absent
User-Agent). It is the wrong status *and* the wrong `detail` type for any body
validation failure.

## Where

`rust/crates/submate-server/src/lib.rs` — the `jellyfin_webhook` handler /
its `Json<JellyfinWebhookPayload>` extraction. Use a custom rejection (e.g. a
`JsonRejection` mapping or a wrapper extractor) so both the syntactic-parse and
the missing-field cases yield **422** + `{"detail": [ ... ]}`.

## Falsifies

`cargo test -p submate-server jellyfin_webhook_bad_body_is_422` (added with the
`port-server-jellyfin-webhook` wiring):

- POST `/webhooks/jellyfin` with body `{"NotificationType":"ItemAdded"}` (no
  `ItemId`) and **any** User-Agent (including a missing or non-Jellyfin one)
  returns **HTTP 422**, and `body["detail"].is_array()` — never 400 with a
  string `detail`, never the UA-rejection 400.
- POST with a syntactically malformed JSON body (e.g. `{`) also returns **422**
  with a list-valued `detail` (not axum's default 400 for a parse error).
- A *structurally valid* body with a bad/missing User-Agent still returns the
  **400** UA-rejection `{"detail": "Invalid request - not from Jellyfin server"}`
  (string `detail`), proving the 422 gate is body-validation-scoped, not
  blanket.

Robust assertion (avoid pinning version-specific `msg`/`loc`/`url`/`input`):

```rust
assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY); // 422, not 400/500
assert!(body["detail"].is_array(), "FastAPI validation detail is a list");
```
