# dive-syntax

A TLA+ lexer and parser that accepts exactly what SANY accepts and produces a lossless concrete syntax tree.

This crate is part of [dive](https://github.com/Chelis-Lang/dive), a native Rust
TLA+ toolchain whose model checker reports the same results as TLC. Most users
want the `dive-cli` command-line tool or the `dive-api` library.
API documentation is on [docs.rs](https://docs.rs/dive-syntax).
