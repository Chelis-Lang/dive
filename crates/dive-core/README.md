# dive-core

The dive execution core: the hash-consed value store, the plan compiler, the interpreter and TLC's successor-generation rules for TLA+ specifications.

This crate is part of [dive](https://github.com/Chelis-Lang/dive), a native Rust
TLA+ toolchain whose model checker reports the same results as TLC. Most users
want the `dive-cli` command-line tool or the `dive-api` library.
API documentation is on [docs.rs](https://docs.rs/dive-core).
