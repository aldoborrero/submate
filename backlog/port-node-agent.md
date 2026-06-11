# Node agent pull-loop

**blocked-by:** port-node-dispatcher, port-proto-types, port-server-node-api

## what
The `submate node --server <url>` agent: register(capabilities) → loop { long-poll request-work → GET audio → Dispatcher → POST progress → POST result → heartbeat }. Reconnect/backoff on server unavailability.

## where
`rust/crates/submate-node/src/lib.rs` using `submate-proto` over `reqwest`.

## why
The worker half of the topology; adding a machine = running this agent.

## falsifies
`cargo test -p submate-node agent_pull_loop` — against a mock server (wiremock), the agent registers, pulls one job, fetches audio, and POSTs a result; on a 204 it long-polls again.
