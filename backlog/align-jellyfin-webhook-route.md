# align: Jellyfin webhook route is /webhooks/jellyfin, payload uses PascalCase aliases

## contract
ROUTE SIGNATURE (path) + request JSON shape (serde rename). Must match Python
byte-for-byte.

## drift

Two related drifts on the Jellyfin webhook contract.

### 1. Wrong route path in the port backlog

`backlog/port-server-jellyfin-webhook.md` ("what") names the route
`/jellyfin/webhook`. The Python source defines it differently:

`submate/server/handlers/jellyfin/router.py`:
```python
router = APIRouter(prefix="/webhooks", tags=["jellyfin"])

@router.post("/jellyfin")
async def jellyfin_webhook(...): ...
```

The mounted path is therefore **`POST /webhooks/jellyfin`**, not
`/jellyfin/webhook`. This is corroborated by the core router's own published
endpoint map in `submate/server/handlers/core/router.py`:
```python
"endpoints": { ... "jellyfin": "/webhooks/jellyfin", ... }
```

An implementer following the port backlog literally will mount the wrong path
and silently break Jellyfin webhook delivery while passing any test that also
hardcodes the wrong path. (Note: the round-orchestration contract blurb also
lists `/jellyfin/webhook`; the Python source is authoritative.)

### 2. Webhook payload PascalCase serde-rename contract is undocumented

`submate/server/handlers/jellyfin/models.py`:
```python
class JellyfinWebhookPayload(BaseModel):
    notification_type: str       = Field(alias="NotificationType")
    item_id: str                 = Field(alias="ItemId")
    item_type: str | None        = Field(default=None, alias="ItemType")
    name: str | None             = Field(default=None, alias="Name")
    server_id: str | None        = Field(default=None, alias="ServerId")
    model_config = {"populate_by_name": True}
```

The Rust `submate-jellyfin` crate already carries serde renames for the
Jellyfin **REST client** models (`Id`, `Path`, `Name`, `Policy`,
`IsAdministrator`), but the **webhook payload** struct does not exist yet and
no backlog item pins its wire field names. The over-the-wire keys Jellyfin
sends are PascalCase (`NotificationType`, `ItemId`, `ItemType`, `Name`,
`ServerId`); deserializing with snake_case field names will fail on every real
payload. `populate_by_name=True` means the Python model also accepts snake_case
input, so the Rust struct should accept both (serde `alias`) but must
deserialize the PascalCase form.

## what to fix
- When porting the Jellyfin webhook route, mount it at `POST /webhooks/jellyfin`
  (router prefix `/webhooks`, path `/jellyfin`), and amend
  `backlog/port-server-jellyfin-webhook.md` accordingly.
- Define the `JellyfinWebhookPayload` Rust struct with serde field names:
  `NotificationType`→notification_type, `ItemId`→item_id,
  `ItemType`→item_type (optional), `Name`→name (optional),
  `ServerId`→server_id (optional), accepting snake_case as an `alias`.

## where
`rust/crates/submate-server/src/lib.rs` (route),
`rust/crates/submate-jellyfin/src/lib.rs` (payload struct),
`backlog/port-server-jellyfin-webhook.md` (path correction).

## falsifies
1. `rg -F '/webhooks/jellyfin' rust/crates/submate-server/src/lib.rs` matches and
   `rg -F '/jellyfin/webhook' rust/crates/submate-server/src/lib.rs` does NOT.
2. `cargo test -p submate-jellyfin webhook_payload_pascalcase`: deserializing
   `{"NotificationType":"ItemAdded","ItemId":"abc","ItemType":"Movie"}` yields
   `notification_type == "ItemAdded"`, `item_id == "abc"`,
   `item_type == Some("Movie")`, and a payload missing `ItemId` fails to
   deserialize (ItemId is required, matching the Python non-optional field).
