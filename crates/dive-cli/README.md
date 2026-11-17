# dive-cli

The `dive` command-line tool: parse, analyze, translate PlusCal, model-check,
simulate and validate traces for TLA+ specifications. It also installs a `tlc`
command that accepts TLC's common checking options. Installing it needs Rust
1.99 or later and a C compiler.

```sh
cargo install dive-cli --locked
dive check Spec.tla
```

This crate is part of [dive](https://github.com/Chelis-Lang/dive), a native Rust
TLA+ toolchain whose model checker reports the same results as TLC. See the
[user guide](https://github.com/Chelis-Lang/dive/tree/main/docs/user) for
commands and options; Rust callers can embed the `dive-api` library instead.
