# Vendored Examples models

The models of `tests/next_examples.rs` (the first slice of milestone CM1), copied
verbatim from [tlaplus/Examples](https://github.com/tlaplus/Examples) at `6a7b26a`, the
commit the conformance corpus pins. Only each model's cfg, its root module and the
modules of its own directory that it extends or instantiates are copied. They are
MIT-licensed; see `LICENSE.md`.

The TLC results they are compared with are the harness goldens in
`conformance/golden/tlc-2026.10.06.014338/examples/`.
