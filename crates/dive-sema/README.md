# dive-sema

Semantic analysis of TLA+ specifications that accepts exactly what SANY accepts: module loading, name resolution, arity and level checking. It bundles the standard modules, CommunityModules and TLAPS modules.

This crate is part of [dive](https://github.com/Chelis-Lang/dive), a native Rust
TLA+ toolchain whose model checker reports the same results as TLC. Most users
want the `dive-cli` command-line tool or the `dive-api` library.
API documentation is on [docs.rs](https://docs.rs/dive-sema).
