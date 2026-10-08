# Vendored Examples models

`specifications/SpecifyingSystems/CachingMemory/MCWriteThroughCache` (its cfg, its
root module and the modules of its own directory that it extends), copied verbatim
from [tlaplus/Examples](https://github.com/tlaplus/Examples) at `6a7b26a`, the
commit the conformance corpus pins. It is MIT-licensed; see `LICENSE.md`.

`tests/fingerprints.rs` checks it in the default profile: dive's former two-level
state fingerprint merged 64 of its 5,196 states (issue #42). The TLC result it is
compared with is the harness golden in
`conformance/golden/tlc-2026.10.06.014338/examples/`.
