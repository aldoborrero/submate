# align: LanguageCode ISO-639-2 (T *and* B) reverse-lookup parity

**relates-to:** submate-lang `parity::lang_conversions`,
`rust/fixtures/capture/capture_lang.py`

## what

Pin the **reverse** ISO-639-2 lookup contract for `LanguageCode` against a
Python golden, for both the terminological (`/T`) and bibliographic (`/B`)
codes. Today the golden + parity test prove every variant's *forward*
conversions (`to_iso_639_2_t` / `to_iso_639_2_b`) and round-trip only the
`iso_639_1` and `name_en` strings back through `from_string`. The
**639-2 codes are never fed back through any `from_*` lookup** against a
captured Python value, so the dedicated B-code matching clause has no
golden-backed parity proof.

Spec is `submate/language.py`:

- `LanguageCode.from_iso_639_2(code)` matches `lang.iso_639_2_t == code`
  **or** `lang.iso_639_2_b == code`.
- `LanguageCode.from_string(value)` matches `iso_639_1`, `iso_639_2_t`,
  `iso_639_2_b`, `name_en`, or `name_native` (lower/stripped).

Rust port is `rust/crates/submate-lang/src/lib.rs`
(`from_iso_639_2`, `from_string`) — the implementation already has the
`e.iso_639_2_b == Some(v)` clause, so this item *pins* that clause, it does
not (today) prove it wrong.

### why this is a distinct contract that can silently drift

For ~20 languages the /T and /B codes diverge, and the /B form is the one
Jellyfin/Bazarr/embedded media commonly tag (`ger`, `chi`, `fre`, `cze`,
`dut`, `gre`, `baq`, `per`, `arm`, `ice`, `geo`, `mao`, `mac`, `may`, `bur`,
`rum`, `slo`, `alb`, `wel`, `tib`). The B-code branch in `from_iso_639_2` /
`from_string` exists *only* to resolve these. A future edit that drops the
`iso_639_2_b == Some(v)` clause (or transposes a T/B pair in the table such
that the B-code resolves to the wrong variant, or to `None`) would:

- still pass `parity::lang_conversions` (it only round-trips `iso_639_1` and
  `name_en`), and
- still pass the inline `iso_639_2_b_divergences` unit test (forward only)
  and `round_trips_and_none` (which checks `from_iso_639_2("ger")` for
  German *only*).

So a real regression in the B→variant direction for any of the other ~19
divergent languages is currently invisible. The forward `to_iso_639_2_b`
column proves the table *holds* `ger`, but nothing proves `from_*("ger")`
*returns* German under the golden.

## where

- `rust/fixtures/capture/capture_lang.py` — add two columns per row:
  `from_iso_639_2_t` and `from_iso_639_2_b`, each computed as
  `LanguageCode.from_iso_639_2(<code>).name` (or `None` when the code is
  `None`, i.e. the `NONE` member). Optionally also `from_string` of the same
  B-code if a single column is preferred — but `from_iso_639_2` is the
  narrower spec and the one with the dedicated B clause.
- `rust/fixtures/lang/lang_conversions.json` — regenerate so each of the 102
  rows carries the two new round-trip columns.
- `rust/crates/submate-lang/tests/parity.rs` (`lang_conversions`) — assert,
  per row, that
  `member_name(LanguageCode::from_iso_639_2(Some(<iso_639_2_t>)))` and
  `member_name(LanguageCode::from_iso_639_2(Some(<iso_639_2_b>)))` equal the
  golden `from_iso_639_2_t` / `from_iso_639_2_b` columns (and resolve to
  `None` for the `NONE` row, whose codes are null).

This is append-only to the existing golden + test; no production code change
is expected (the Rust implementation already matches). The point is to make
the B-code reverse contract *falsifiable*.

## falsifies

In `submate-lang`'s `parity::lang_conversions`, after the new columns land,
both of these must hold for every divergent row (and round-trip back to the
named variant, never `None`):

- `LanguageCode::from_iso_639_2(Some("ger"))  == LanguageCode::GERMAN`
- `LanguageCode::from_iso_639_2(Some("chi"))  == LanguageCode::CHINESE`
- `LanguageCode::from_iso_639_2(Some("tib"))  == LanguageCode::TIBETAN`
- `LanguageCode::from_iso_639_2(Some("deu"))  == LanguageCode::GERMAN`  (T side)
- `LanguageCode::from_iso_639_2(Some("zho"))  == LanguageCode::CHINESE` (T side)

Concrete falsifier: delete the `|| e.iso_639_2_b == Some(v)` clause from
`from_iso_639_2` (and/or `from_string`) in
`rust/crates/submate-lang/src/lib.rs`. With the new golden columns,
`cargo test -p submate-lang lang_conversions` must fail with a `from_iso_639_2_b`
mismatch (`ger` → `NONE` instead of `GERMAN`, etc.). Without the new columns —
the state today — the same deletion leaves the suite green. That gap is the
bug this item closes.
