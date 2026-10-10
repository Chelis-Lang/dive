# Replacing a PlusCal translation block

`dive translate --rewrite` returns a replacement document in memory. It leaves
its input file unchanged. The supported checksum contract is explicit:
`--checksums tla` records a CRC for generated TLA and disables the algorithm CRC.
Full algorithm checksum generation is pending; the default `--checksums full`
returns a named unsupported result rather than inventing a checksum.

Save this P-style source as `Rewrite.tla`:

```tla
---- MODULE Rewrite ----
EXTENDS Integers
(* --algorithm A
variable x = 0;
begin
  Start: x := 1;
  Finish: skip;
end algorithm; *)
\* Keep this comment.
====
```

The equivalent C-style comment is:

```tla
(* --algorithm A {
  variable x = 0;
  { Start: x := 1; Finish: skip; }
} *)
```

Run:

```sh
dive translate Rewrite.tla --rewrite --checksums tla --output json > plan.json
```

On accepted generation, `result` is `replacement`; `replacement.module_source`
contains the admitted module, and `replacement.span` describes the original byte
interval. The header has `chksum(pcal) \in STRING` and `chksum(tla) = "..."`.
`algorithm_checksum_verified` is false, `body_checksum_verified` is true, and
`model_proved` and `exhaustive` are false. Translation is not model checking.
To inspect the generated document separately without overwriting the source:

```sh
mkdir -p generated
python3 -c 'import json; d=json.load(open("plan.json")); assert d["accepted"]; open("generated/Rewrite.tla", "w").write(d["replacement"]["module_source"])'
dive analyze generated/Rewrite.tla --output json
```

An unchanged existing body with a matching CRC can be replaced. A modified body,
missing checksum or disabled body checksum needs `--replace-modified`; otherwise
replacement is refused. Existing algorithm CRC values appear as **unverified**
selection evidence and are disabled in the replacement. Unsupported generation
or enclosing-module rejection returns no replacement. `--write` is unsupported;
there is no implicit backup or file overwrite.

Both source syntaxes use the same generator support boundaries as
[PlusCal translation](pluscal-normalization.md) and
[process/procedure fairness](pluscal-procedure-fairness.md). This editor does not
expand that scope. It refuses multiple algorithm comments, duplicate/reversed
sections, unproved marker placement and marker-like text outside a proved line
comment. All spans are UTF-8 byte offsets. The source and output limit is 4 MiB;
Rust callers may lower it. Defaults preserve surrounding bytes and use the first
source newline style for generated lines. `--compat=tlc` normalizes LF, but the
partial checksum policy is explicitly reported as `tlc+overrides`, not full TLC
checksum parity.

Rust callers can review the plan without disk side effects:

```rust
use dive_api::{ChecksumPolicy, RewriteOptions, RewriteRequest, rewrite_source};
let source = "---- MODULE R ----\nEXTENDS Integers\n(* --algorithm A variable x=0; begin L:x:=1; E:skip; end algorithm; *)\n====\n";
let request = RewriteRequest {
    editing: RewriteOptions {
        checksums: ChecksumPolicy::TranslationOnly,
        ..RewriteOptions::default()
    },
    ..RewriteRequest::default()
};
let report = rewrite_source("R.tla", source, &request);
assert!(report.accepted());
assert_eq!(report.to_json()["model_proved"], false);
```

`rewrite_file` performs a bounded UTF-8 read; `rewrite_source` generates and
admits the module. `dive_pluscal::rewrite::rewrite_document` is a lower-level pure
editor of a **supplied** body: its plan alone certifies neither generation nor
TLA acceptance. The CRC is noncryptographic and omits line separators, matching
the reference. It is not a semantic equivalence check or a proof.

Evidence lives in
[`rewrite-acceptance`](../crates/dive-pluscal/rewrite-acceptance/README.md).
Exact primary bytes from supplied primary-body controls are separate from native
emitted bytes/admission. All three translator-success/SANY-invalid primary
artifacts remain visible and uncredited. The pinned fair-algorithm checksum bug
[D-020](tlc-differences.md#d-020-fair-algorithm-checksums-do-not-detect-source-edits)
is recorded separately: no corrected algorithm-checksum default is implemented.
The full AST CRC must serialize the name-fixed, post-translation exploded AST
(as pinned Validator does). That CRC, exact pretty-print/file rewriting conventions and mapping-format
contracts remain pending under [ADR 0047](decisions/0047-bounded-translation-block-editing.md).
