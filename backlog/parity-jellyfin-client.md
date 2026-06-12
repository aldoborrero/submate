# Parity falsifier for the Jellyfin REST client

**blocked-by:** none (submate-jellyfin `JellyfinClient` is already implemented in
`rust/crates/submate-jellyfin/src/lib.rs`; submate-config and submate-paths are
ported). This item adds the missing golden-backed parity tests — it does NOT
re-port the client.

## what

`submate/media_servers/jellyfin.py` (`JellyfinClient`) is already ported but only
has two thin unit tests (`auth_header_uses_mediabrowser_token_form`,
`is_configured_requires_url_and_key`). The request URLs/headers/query params and
the JSON-response parsing — all of which the contract requires to match Python
byte-for-byte (it is the integration-boundary wire format) — are currently
**unfalsified**. `tests/test_jellyfin.py` pins this behavior on the Python side;
nothing pins it on the Rust side. Add golden-backed parity tests.

Two independent, one-worktree-sized units (split so request-shape work and
response-parsing work don't touch the same test module):

### Unit A — request-shape parity (URLs, headers, query params)

Drive each public method against a mock HTTP server (`wiremock`, a workspace
dev-dep) seeded with the canned response bodies, and assert the **outgoing**
requests exactly match the Python client. The exact request surface to pin
(from `submate/media_servers/jellyfin.py`, all carrying
`Authorization: MediaBrowser Token=<api_key>` and a 10s timeout):

- `connect()`        -> `GET  {server}/Library/VirtualFolders`
- `admin_user_id()`  -> `GET  {server}/Users`
- `get_file_path(id)`-> `GET  {server}/Users/{admin_id}/Items/{item_id}`
- `refresh_item(id)` -> `POST {server}/Items/{item_id}/Refresh?Recursive=true`
- `refresh_library(name)` -> `GET {server}/Library/VirtualFolders` then
  `POST {server}/Items/{library_id}/Refresh?Recursive=true`

Note for the implementer: the auth header is the single most pinned fact —
Python's `test_jellyfin_connect_uses_authorization_header` asserts
`{"Authorization": "MediaBrowser Token=secret"}` exactly. The `Recursive=true`
query param is sent as a **string** `"true"`, not a bool.

### Unit B — response-parsing parity (admin selection, path/library lookup)

Feed each parser the golden Jellyfin JSON response bodies and assert the derived
value matches the Python semantics:

- admin-user selection: from a `/Users` array, pick the first user with
  `Policy.IsAdministrator == true`, return its `Id`; raise `NoAdminUser` when
  none (Python `_get_admin_user_id`). Missing/absent `Policy` -> not admin.
- file path: from a `/Users/{admin}/Items/{item}` object, return `Path`; a
  missing or empty `Path` raises `NoFilePath(item_id)` (Python truthiness check —
  empty string counts as "no path").
- library lookup: from a `/Library/VirtualFolders` array, find the entry whose
  `Name == library_name`, take its `Id`; **no match is not an error** — Python
  logs a warning and returns without refreshing (so `refresh_library` of an
  unknown name must NOT issue the POST and must NOT raise).

## where

`rust/crates/submate-jellyfin/src/lib.rs` (add `#[cfg(test)] mod parity { ... }`
alongside the existing `mod tests`). Use `wiremock` for Unit A. Unit B can call
the existing private `serde`-derived structs (`User`, `Item`, `VirtualFolder`)
directly — split the admin-pick / path-extract / library-match selection logic
into small private free functions if needed so they're testable without HTTP.

Do NOT touch `rust/fixtures/` — it is denylisted. The goldens named below must be
captured first (see below); until they exist, gate the tests behind a
"fixture missing -> eprintln skip" guard (the same pattern as the existing
`extract_pcm_sha` test in `submate-media`) so the test arms itself the moment the
fixtures land.

## why

The Jellyfin client is the webhook pipeline's only way to resolve an `ItemId` to
a real file path and to kick library refreshes. A wrong URL, a dropped
`Recursive=true`, a mis-cased header, or picking the wrong admin user all fail
silently against a real server and are exactly the byte-for-byte wire details the
porting contract calls out for integration boundaries.

## falsifies

`cargo test -p submate-jellyfin parity` —

- **Unit A** `parity::request_shapes`: with `wiremock` serving the golden
  response bodies, each method's recorded request (method, path, query, the
  `Authorization` header) equals the golden request descriptors in
  `rust/fixtures/jellyfin/requests.json` (an ordered list of
  `{method,path,query,auth}` per call, captured from the Python client driven
  against a `responses`/`requests-mock` stub).
- **Unit B** `parity::response_parsing`: feeding the golden bodies
  `rust/fixtures/jellyfin/users.json`,
  `rust/fixtures/jellyfin/item.json`, and
  `rust/fixtures/jellyfin/virtual_folders.json` yields admin id `admin123`,
  path `/media/movies/test.mkv`, and library id matching `Movies`,
  byte-for-byte equal to the Python-derived values in
  `rust/fixtures/jellyfin/expected.json`; the no-admin and empty-`Path` cases
  raise `NoAdminUser` / `NoFilePath`, and the unknown-library case issues no POST.

  requires fixtures (capture first — `rust/fixtures/` is denylisted for the port,
  add a `capture/capture_jellyfin.py` driving the Python `JellyfinClient` against
  a `responses`-stubbed server and dumping both the recorded requests and the
  canned response bodies):
  - `rust/fixtures/jellyfin/requests.json`
  - `rust/fixtures/jellyfin/users.json`
  - `rust/fixtures/jellyfin/item.json`
  - `rust/fixtures/jellyfin/virtual_folders.json`
  - `rust/fixtures/jellyfin/expected.json`

  The response-body goldens should mirror the shapes already used in
  `tests/test_jellyfin.py` (admin user `{"Id":"admin123","Policy":{"IsAdministrator":true}}`,
  item `{"Path":"/media/movies/test.mkv"}`, folders
  `[{"Name":"Movies","Id":"library-1"},{"Name":"TV Shows","Id":"library-2"}]`)
  so the capture is deterministic and matches the existing Python test data.
