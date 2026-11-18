# dive-model

Binds a lowered TLA+ specification to its TLC model configuration: constants, overrides, constant definitions and checking roles, in TLC's order.

This crate is part of [dive](https://github.com/Chelis-Lang/dive), a native Rust
TLA+ toolchain whose model checker reports the same results as TLC. Most users
want the `dive-cli` command-line tool or the `dive-api` library.
API documentation is on [docs.rs](https://docs.rs/dive-model).
