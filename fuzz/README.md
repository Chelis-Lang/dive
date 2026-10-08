# Fuzzing

`cargo-fuzz` targets. They need a nightly toolchain, so this crate is not a
workspace member, and CI does not run it (charter: long fuzzing runs by hand).

```sh
cargo install cargo-fuzz
cargo +nightly fuzz run parse                 # from the repository root
cargo +nightly fuzz run parse -- -max_total_time=600
```

Targets:

- `parse`: `dive_syntax::parse_module` on arbitrary bytes. It must not panic,
  the CST text must equal the input, and every diagnostic span must lie inside
  the input.

Seed corpora are useful: copy `crates/dive-syntax/tests/sany-cases/*.tla` into
`fuzz/corpus/parse/`, which git ignores.

The stable test suite runs the same properties with `proptest`
(`crates/dive-syntax/tests/roundtrip.rs`).
