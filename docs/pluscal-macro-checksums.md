# Macro Full checksums

Bounded nonrecursive macros can be expanded and serialized for a Full
translation-block replacement. Save this as `MacroFull.tla`:

```tla
---- MODULE MacroFull ----
EXTENDS Integers
(* --algorithm A
variable x = 0;
macro Set(v, a) begin v := a; end macro;
begin Start: Set(x, 1 + 2); Finish: skip; end algorithm; *)
====
```

Run `dive translate MacroFull.tla --rewrite --checksums full --output json`.
For an admitted replacement the report contains `result: "replacement"`,
`accepted: true`, both checksum-verification flags, and the complete updated
module in `replacement.module_source`. Input files remain unchanged.
`model_proved` and `exhaustive` are false: generation, module admission and
reference CRC derivation do not prove the algorithm correct.

The C-style equivalent is:

```tla
---- MODULE MacroFullC ----
EXTENDS Integers
(* --algorithm A {
variable x = 0;
macro Set(v, a) { v := a; };
{ Start: Set(x, 1 + 2); Finish: skip; }
} *)
====
```

Both profiles use the same bounded serializer; add `--compat=tlc` for the
global compatibility profile. Automatic labels and the existing source/CLI
fairness settings retain their requested/effective option context. Source
declarations inside a previous translation block remain refused before masking.
`--checksums tla` explicitly disables the unproved algorithm CRC for otherwise
supported generation and is reported as an override under compatibility.

## Rust callers and raw evidence

`dive_api::rewrite_source` with `RewriteRequest::default()` generates, edits
and admits the enclosing module in memory. The Cargo-owned public API example exercises that admission contract:

```sh
cargo run -p dive-api --example pluscal_source_context -- MacroFull.tla rewrite declared tlc
```

On this fresh module it reports the same admitted Full replacement as the CLI.
Its explicit `replace_modified` grant permits replacing an existing modified
section; use the default `RewriteRequest` without that grant when requiring
unchanged checksums. The [example source](../crates/dive-api/examples/pluscal_source_context.rs)
constructs the request and calls `rewrite_source` directly. A Cargo-owned raw
serialization example is also available:

```sh
cargo run -p dive-pluscal --example macro_checksum -- MacroFull.tla
```

The example calls `select_rewrite` and `algorithm_checksum_at`, returns the
serialized post-expansion AST and CRC, and always reports `artifact_admitted`,
`model_proved` and `family_certified` as false. Exact raw bytes can match a
reference artifact that SANY rejects. Use the public rewrite API to require
module admission, and a separately configured `dive check` to check a model.

Single-line expression substitutions, nested earlier-declared macros and
supported `with` contexts retain the pinned logical token layout. Source spans
still name the original argument or macro definition. Unchanged junctions
retain their original columns. Multiline arguments, multiline macro bodies,
substituted `/\` or `\/` junction columns, duplicate formals, recursive calls,
unproved definition scopes, procedures and scoped name fixes are named refusals. See the separate [bounded DEFINE path](pluscal-definitions.md).
Existing finite-process ownership restrictions still apply.

The embedded selector handles ASCII-star comment decoration such as
`(** --algorithm ... **)` without changing original source bytes. A standalone
algorithm parser still requires EOF. Embedded selection now uses the exact completely parsed C/P algorithm end,
matching the reference when ordinary trailing prose follows that end. The
original two raw/four public false-rejection observations remain archived.
Missing algorithm endings, unterminated quoted/comment lexical state and global
resource exhaustion still reject; standalone parsing does not ignore trailing tokens.

Within an owned algorithm comment, selection follows the pinned translator's
line order. If fair and unfair markers share a line, its unfair marker wins;
a later line cannot override an earlier fair algorithm. Split-line, tab and
multiple-space separators after `--fair` are parsed normally; malformed marker
prefixes reject. This rule is identical in both profiles. Ignored trailing
algorithm-shaped text remains in the original comment, without being translated.

Generated declarations are inserted immediately after the complete algorithm
closing line, before user definitions that refer to generated variables or `pc`.
Only ASCII whitespace through 0x20 or a `\*` line comment may follow the
closing delimiter on that line, matching pinned translation. Inline definitions,
another block comment and non-ASCII spacing such as NBSP reject before
generation. LF/CRLF/tab and permitted line-comment contexts have paired controls.
Both C/P forms and preexisting translation sections have clean pinned ordering
controls; genuine unknown names still reject. The original `Lock` public
ordering failures are retained, without upgrading raw CRC equality into proof.

All parser, expanded-node, action and serialization-output quotas remain
unchanged. Reaching a bound returns a typed resource failure without a partial
checksum or replacement. File writes, exact reference pretty-printing and
source-map formatting remain outside this increment.

Reference CRC equality is not a certificate of source identity. In particular,
the pinned `--fair algorithm` header hashes only `fair`; the report retains
`fair_header_crc_only: true`. See [D-020](tlc-differences.md#d-020-fair-algorithm-checksums-do-not-detect-source-edits)
and [source option context](pluscal-source-options.md). No checksum default
correction is introduced here.

The [retained study](../crates/dive-pluscal/macro-checksum-acceptance/README.md)
separates clean translator output, development-only AST stages, SANY,
configured Validator, native raw serialization, public admission and finite
model/witness checks. It keeps every reference rejection and invalid artifact.
The original 240-row primary selection contains 28 SANY-invalid translated
artifacts; none earns artifact or model-proof credit. `Lock` and `Barriers`
are retained whole sources without general corpus certification.

The inherited marker-selection defect is tracked as #192; exact original main
selector bytes and separately qualified pre-repair runtime observations are
retained in the study. The final expanded selection passes all 696 public
profile requests, with every invalid reference artifact and unsupported case
still visible. Full translator/family certification remains pending.
