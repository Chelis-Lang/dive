# Changelog

Notable user-facing changes are recorded here. The first release has not been
published. Earlier pre-release development entries remain in git history.

## Unreleased

- Prepare native Linux x86_64/aarch64 release archives, SHA-256 checksums and
  bundled-library license notices. No version bump, tag or publication is part
  of this preparation.
- Replace historical workstream narratives with the first-value release gates
  and a user-oriented README.

## 0.1.0

This is the curated draft for the first release, **not a published release**.
The [release gates](docs/RELEASE.md) must pass before it is tagged.

### Added

- Native TLA+ parsing and semantic analysis, supported PlusCal translation,
  and explicit-state checking without a JVM.
- Safety and supported temporal checking, including fairness, with clear
  refusals for unimplemented features and explicit run-budget stops.
- Counterexamples independently validated against their specifications.
- Schema-3 JSON operation reports, pinned by fixtures for seven Core checking
  outcomes, with one shared API version constant for validation errors.
- Per-command help without input files, source revision information in
  `--version`, and human checking headers naming engine and workers. The
  unimplemented `lsp` command is omitted from help.
- Embedded standard modules, TLAPS and CommunityModules, with local modules and
  explicit search paths taking precedence. Analysis JSON identifies bundled
  sources; ordinary imports need no extra library flags.
- Linux x86_64/aarch64 archives with checksums, both command entrypoints, and
  the bundled libraries' license and provenance notices.

### Changed

- Core builds finite sets canonically, removing TLC construction-history and
  receiver-authority tracking. Default mixed structured sets have consistent
  mathematical identity; unproved symbolic identities still refuse (D-028).
- Core evaluates applied LET-recursive functions with lexical captures and pure
  recursive applications under prime. Separate callee frames preserve INSTANCE
  substitutions; escapes, unproved captures and effectful primed functions refuse.
- Applied recursive functions resolve enclosing pure operator callbacks, including
  lambdas and forwarded parameters, in their lexical context. Unreachable
  captures, effects and unproved recursive calls retain named refusals.
- Restore top-level mutually recursive operators used by Nano's balance
  calculations. Key recursive compilation by lexical activation and INSTANCE
  bindings, preserving holding and violated invariants; mutually recursive
  captured local definitions retain their named refusal.
- Effect-safe recursive Boolean action guards avoid duplicate predicate witnesses
  and unnecessary Boolean alternatives (D-029). Default differences are precisely
  registered; nominal TLC retains documented count and short-circuit gaps.
  Recursive calls and arguments stop safely at 64 activations and poll budgets.
- Native words after empty product/record Init branches validate through actual
  pure Init emissions. Forged first states reject; earlier errors, partial states
  and interrupted replay cannot prove membership. Partial-Init witnesses and
  existing trace versions keep their meanings.
- Core temporal reads reuse fully materialized finite-set conversions within
  the existing bounded cache. Compound membership reuses its materialized-element
  admission proof while retaining query and interruption checks. These changes
  establish no release speed result.
- Benchmark runs stop before a later arm after an invalid or incomplete sample,
  retain physical process output and report actual worker use. Core selection is
  explicit; incomplete runs cannot supply a complete-suite performance result.

### Compatibility and limits

- Reference remains the default checker; `--engine core` selects Core without
  fallback. First-release Core coverage and speed must meet the release gates.
  Unimplemented requests refuse by name; budget stops are not success.
- `--compat=tlc` retains inexpensive compatibility switches and documents its
  gaps; exact reproduction of TLC implementation artifacts is not promised.
- EWD998ChanID imports resolve, but checking may retain its specific named
  VIEW/liveness or recursive LET refusal. The bare-use smoke requires the other
  nine original Examples to check successfully; this gives no Core parity credit.
- Distributed workers, debugging, recovery breadth, coverage and LSP are outside
  the first release's acceptance scope. Existing bounded implementations do not
  imply complete support for those modes.
