# Bundled module libraries

These TLA+ sources are embedded in dive as the final module-search fallback.
Local files, explicit library/module paths, standard modules and modules inside
the root file take precedence. No runtime download or JVM is needed.

- The project maintains `dive/FramedFunctions.tla` under the repository MIT license.
  Release archives also ship the source in `share/dive/tla/` for TLC users.
- `community-modules/`: all 34 root `.tla` files from
  `CommunityModules-202610040242.jar`, SHA-256
  `b7fcd727fc5135a628e56d5015651c74ba70681bbaf88df1cd59e9afa64215ad`,
  repository commit `78b98ae9939d2fe7a6615e9a4b21dd6c23ef8ff3`. MIT;
  see `community-modules/LICENSE`.
- `tlaps/`: all 14 `.tla` files from tlapm `library/` at
  `d8b9747a1dce9b4816bcc1ff130ccf7a3af2ce37`. BSD-2-Clause;
  see `tlaps/LICENSE`.

The third-party libraries are copied verbatim from the pins in `conformance/oracles.toml` and
`conformance/corpora.toml`. To update, replace the source files and licenses
from the new pins, update `src/bundled.rs`, and run the bundled-module tests
and bare-use smoke. Keep the standard-module copies at their existing priority.
Release archives must include this README and both license files.
