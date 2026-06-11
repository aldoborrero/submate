# Audio payload transfer to nodes

**blocked-by:** port-server-node-api, port-media-extract

## what
The server (which runs where the media is) extracts PCM via submate-media and serves it: the request-work response carries `audio_url`; `GET /jobs/{id}/audio` streams the s16le/mono/16k PCM. Nodes fetch rather than the server inlining bytes in JSON. Bazarr-uploaded audio is relayed the same way.

## where
`rust/crates/submate-server/src/lib.rs`.

## why
Nodes have no media access by design; the server is the only place audio is produced.

## falsifies
`cargo test -p submate-server audio_transfer` — enqueue a job with extracted PCM; `GET /jobs/{id}/audio` returns bytes whose sha256 matches the source extraction.
