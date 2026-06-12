# UX: live transcription progress

## what
While `transcribe --sync` waits, render live progress from the node's `Progress`
events (a spinner + percentage via `indicatif`), finishing with the result line.
Fall back to plain periodic lines when stderr is not a TTY.

## where
`rust/crates/submate-server/src/lib.rs` (expose a per-job progress subscription
from `NodeCoordinator`, alongside the existing result waiter) +
`rust/crates/submate-cli/src/main.rs` (subscribe and render).

## why
On a real-length video the user currently sees nothing for minutes until
"Processed", even though the node already posts `Progress`.

## falsifies
`cargo test -p submate-server coordinator_progress_subscription` — posting a
sequence of `Progress` events (0 → 100) is delivered to a subscriber in order;
and `cargo test -p submate-cli progress_non_tty_plain` asserts non-TTY mode emits
plain progress lines. Mockable, no model needed.
