# Port the Jellyfin REST client

**blocked-by:** port-config-schema

## what
Port `JellyfinClient` (connect, get_file_path, refresh_item/library/all) over `reqwest` with the `MediaBrowser Token=` auth header.

## where
`rust/crates/submate-jellyfin/src/lib.rs`.

## why
Used by the webhook handler + the transcribe --refresh-jellyfin CLI flag.

## falsifies
`cargo test -p submate-jellyfin parity::requests` matches the Jellyfin request goldens under `wiremock`.
