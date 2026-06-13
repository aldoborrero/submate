# align: config `custom_regroup` env false-coercion serializes as JSON bool

**relates-to:** submate-config defaults/validators parity (`rust/crates/submate-config/tests/parity.rs`)

## what

Pin the **serialization shape** of `StableTsSettings.custom_regroup` when an
environment value disables regrouping. Python's `parse_regroup`
(`submate/config.py`, `mode="before"` validator on `StableTsSettings`) coerces
any string in the set `{"false", "off", "0", "no", ""}` (case-insensitive) to the
Python bool `False`. Because the field is typed `str | bool`, `model_dump`
emits the **JSON boolean `false`**, not the string `"false"`.

The Rust port already implements this in `deserialize_regroup`
(`rust/crates/submate-config/src/lib.rs`): the disable-set
`"false" | "off" | "0" | "no" | ""` maps to `StrOrBool::Bool(false)`, and
`StrOrBool` is `#[serde(untagged)]` so it serializes as a bare `false`. The
problem is that **no golden fixture or test exercises this branch.** The existing
`config/validators.env` sets
`SUBMATE__STABLE_TS__CUSTOM_REGROUP=cm_sl=84_sl=42++++++1` (a non-disabling
pattern), and the parity test's own doc comment says it only pins "the
`custom_regroup` string passthrough (a non-disabling pattern stays a string)."

So the contract that `false`/`off`/`0`/`no`/empty → JSON `false` is asserted
**only against the Rust port's own copy of the rule**, never against a
Python-captured golden. A refactor that drops a case from the disable-set, makes
the match case-sensitive, or emits the string `"false"` instead of the bool would
pass every current `submate-config` test while silently diverging from Python.

This is a CONFIG-KEYS contract gap (the round's contract #1: "every settings
field name + default" — extended here to the field's *env-coercion serialization
shape*, which is equally load-bearing for byte-for-byte config parity).

### Python evidence (run against the Python SPEC tree)

```
SUBMATE__STABLE_TS__CUSTOM_REGROUP=false  ->  false   (type: bool)
SUBMATE__STABLE_TS__CUSTOM_REGROUP=off    ->  false
SUBMATE__STABLE_TS__CUSTOM_REGROUP=       ->  false
```

(`json.dumps(Config().stable_ts.custom_regroup)` yields the literal `false`,
never `"false"`.)

## where

- Golden: a new capture case, e.g. `rust/fixtures/config/regroup_disabled.env`
  (`SUBMATE__STABLE_TS__CUSTOM_REGROUP=off`) +
  `rust/fixtures/config/regroup_disabled.resolved.json`, emitted by
  `rust/fixtures/capture/capture_config.py` (run against the Python tree — do NOT
  hand-author the resolved JSON). The resolved JSON's `stable_ts.custom_regroup`
  MUST be the JSON literal `false`.
  Note: the empty-value case (`...CUSTOM_REGROUP=` → `false`) cannot ride the
  existing `parse_env` helper in `tests/parity.rs` (its `split_once('=')` yields
  an empty string, which is fine, but capture must drive it from Python). Prefer
  capturing `off` (unambiguous) for the golden and add `false`/`0`/`no`/empty as
  Rust-side `deserialize_regroup` unit cases.
- Test: extend `rust/crates/submate-config/tests/parity.rs` with a
  `regroup_disabled` case that resolves the new `.env` through `Config::from_env`
  and `assert_json_eq`s against the new golden — mirroring the existing
  `validators` test structure (figment `Jail`, `clear_env`, BTreeMap compare).

## why

The disable-coercion of `custom_regroup` is a serialization-shape contract: the
downstream stable-ts regroup layer branches on bool-`false` vs a pattern string,
and any config consumer that re-emits config (e.g. `config show`, a `/config`
dump) must produce the JSON boolean to match Python byte-for-byte. The string
`"false"` is a *truthy* non-empty value that would (a) be re-parsed as a regroup
*pattern* on a round-trip and (b) diverge from the Python golden. Today this is
invisible to the test suite.

## falsifies

`cargo test -p submate-config regroup_disabled` — resolving
`SUBMATE__STABLE_TS__CUSTOM_REGROUP=off` (and, as `deserialize_regroup` unit
cases, `false` / `0` / `no` / empty) through `Config::from_env` yields a config
whose serialized `stable_ts.custom_regroup` equals the Python-captured golden,
i.e. the JSON literal `false` (a boolean), never the string `"false"`. The test
fails if any disable-set case is dropped, made case-sensitive, or serialized as a
string.
