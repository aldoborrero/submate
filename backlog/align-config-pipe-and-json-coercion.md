# Align: config pipe-list / JSON-kwargs / regroup-bool coercion is unimplemented and uncovered

**contract:** CONFIG KEYS (value coercion) — `submate/config.py` field validators
**relates-to:** port-config-validators (the implementation tracker)
**severity:** load-breaking — affects documented env patterns

## divergence

`submate/config.py` defines `mode="before"` field validators that coerce env
**strings** into structured values before validation:

* `parse_pipe_separated_*` (folders, libraries, and the three `subtitle.*` language
  lists): `"a|b|c"` → `["a","b","c"]`, splitting on `|` and stripping whitespace.
* `WhisperSettings.parse_json_kwargs`: `transcribe_kwargs` is a JSON **string**
  (`'{"beam_size": 5}'`) parsed into a dict.
* `StableTsSettings.parse_regroup`: `custom_regroup` accepts `"false"`/`"off"`/`"0"`/
  `"no"`/`""` (case-insensitive) → bool `false`; any other string passes through.

The Rust `submate-config` crate (`rust/crates/submate-config/src/lib.rs`) declares
these fields as plain `Vec<String>` / `Map<String,Value>` / `StrOrBool` with bare
`#[serde(default)]` and resolves env via `Env::prefixed("SUBMATE__").split("__")`.
figment passes env values to serde as **strings**; there is no per-field
`deserialize_with` to replicate the Python coercion. So:

* a pipe-separated list env var does not split — it **errors** the whole load,
* a JSON-string `transcribe_kwargs` would fail to deserialize into a map,
* `custom_regroup=false` (string) would deserialize as the string `"false"`, not bool.

The golden `rust/fixtures/config/validators.resolved.json` already captures the
correct Python output, but **no Rust test consumes it** — `tests/parity.rs` covers
only `parity::defaults` and `parity::env_nesting`, and `nested.env` exercises only
scalar fields (model/device/port/backend/api_key), never a list or kwargs field.
So `cargo test -p submate-config` is green while this contract is entirely unmet.

## falsifier (currently RED)

Add `tests/parity.rs::validators` (the test named in `port-config-validators`'s
falsifier line) that loads `config/validators.env` through `Config::from_env(None)`
inside a `figment::Jail` and exact-diffs against `config/validators.resolved.json`:

```rust
#[test]
fn validators() {
    figment::Jail::expect_with(|jail| {
        jail.clear_env();
        for (k, v) in parse_env("config/validators.env") { jail.set_env(k, v); }
        let cfg = Config::from_env(None).expect("env resolves into Config");
        let actual = serde_json::to_value(&cfg).unwrap();
        assert_json_eq(&actual, &golden("config/validators.resolved.json"));
        Ok(())
    });
}
```

Observed today against `origin/main` (`_base` @ 54a55a0), this panics at
`Config::from_env`:

```
Error { ... path: ["subtitle", "skip_subtitle_languages"],
        kind: InvalidType(Str("en|es|fr"), "a sequence") }
```

i.e. figment refuses to deserialize the pipe string into `Vec<String>` and aborts
the entire load. (The kwargs and regroup divergences are masked behind this first
hard error; all three need fixing.)

## fix (for the implementer of port-config-validators)

Attach `#[serde(deserialize_with = ...)]` to each affected field:

* pipe-lists — split on `'|'`, `trim()`, drop empties; accept an already-parsed
  sequence too (figment file layer / defaults supply a real array).
* `transcribe_kwargs` — if the incoming serde value is a string, `serde_json::from_str`
  it into a `Map`; if already a map, pass through; empty/absent → `{}`.
* `custom_regroup` — string in {false,off,0,no,""} (lowercased) → `Bool(false)`;
  bool passes through; other string → `Str(_)`.

Match Python's permissive branches: a real array/dict/bool arriving from the file
or defaults layer must still deserialize, so the custom deserializers must accept
both the raw-string env form and the already-typed form.

## done when

`cargo test -p submate-config parity::validators` is green (exact match vs
`rust/fixtures/config/validators.resolved.json`) and the existing `parity::defaults`
and `parity::env_nesting` stay green.
