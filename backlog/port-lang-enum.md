# Port language.py LanguageCode to submate-lang

## what
Port the 113-variant `LanguageCode` enum + ISO-639-1/2 (T and B) conversions and name lookups from `submate/language.py`. Hand-roll the table to match Python exactly — do NOT pull a third-party language crate whose tables differ.

## where
`rust/crates/submate-lang/src/lib.rs`.

## why
Used by subtitle/path/config layers; conversions must be exact.

## falsifies
`cargo test -p submate-lang parity::lang_conversions` passes exact-match against `rust/fixtures/lang/lang_conversions.json` for all 113 languages, both directions.
