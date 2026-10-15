# 0084: PlusCal parsing and translation

- **Status:** accepted
- **Date:** 2026-10-12

## Context

TLC ships the PlusCal translator (`pcal.trans`). dive translates PlusCal natively
in `dive-pluscal`, so translation needs no JVM. More than twenty increment ADRs
widened the admitted subset one shape at a time: sequential code, labels,
processes, fairness, macros, procedures, definitions, checksums and block
editing. This ADR records the decisions that remain, without that history.
User-facing detail is in `docs/pluscal-translation.md`,
`docs/pluscal-parser.md` and `docs/user/pluscal.md`.

## Decision

**Pipeline.** Translation runs in separate stages: source parsing, macro
expansion, label and control-flow normalization, generation, and admission of the
enclosing module. `parse`/`parse_at` produce an unnormalized source AST that
keeps the original bytes and half-open byte ranges. Each public call runs on its
own bounded-stack worker, with explicit byte, token, node, depth, action and
output limits. A limit gives a named resource result. A worker failure gives
an internal diagnostic. No partially accepted output is ever returned.

**Admission.** A translation is returned only after the complete generated
module passes dive's native syntax, name and level analysis. Translation never
claims a model proof or exhaustive exploration; schema2 omits those fields. Any construct that is not supported yet is a named refusal,
never an approximation. Correctness is judged against the pinned translator by
comparing output, together with generated-model checking and independent trace
replay. The comparison uses significant tokens and the operator CST. A primary
artifact that SANY rejects earns no agreement.

**Semantics followed.** dive matches the pinned translator's observable output
for the admitted subset:
- label insertion rules: the sequential default, `-label`, `-labelRoot` and
  `-reportLabels`, and the process rule that every missing label needs `-label`;
- `defaultInitValue` for omitted initializers, returned as required
  configuration data;
- simultaneous-assignment snapshots and the program-counter omission rules;
- process ID/domain expressions and scoped symbol disambiguation;
- fairness declarations and label modifiers, with option precedence for `-wf`,
  `-sf`, `-wfNext` and `-nof`;
- macro substitution in source order;
- sequential, nested, recursive and process-owned procedure calls, using stack
  frames rather than unrolling;
- global `define` operators and functions, including LOCAL.

The translator follows the reference stages for scoped name disambiguation,
multiline expression substitution and labels inside branches, rather than
admitting isolated source shapes. Symbolic process domains are translated
without evaluating them or certifying finiteness/disjointness. Generated-operator
collisions and duplicate macro formal parameters remain named refusals. Shared
macro budgets stop amplification before token allocation. Both profiles use the
same translation.

**Editing and checksums.** `dive translate --rewrite` returns an in-memory
replacement plan for the translation block and never writes the input file.
Plain translation also replaces an existing block in its original position,
without preserving stale checksum headers. Generation and final module analysis
use this position so intervening user definitions remain in scope. The
best-effort policy always writes the real generated-body CRC and writes an exact
algorithm CRC only when the procedure-free serializer supplies one. Otherwise
it writes `chksum(pcal) \in STRING`, retaining the pcal clause so pinned SANY/TLC
still detect body changes. Procedure header-transfer emulation is removed; no
unknown algorithm CRC, or debug CRC availability, may block translation/rewrite.
The old `Full` and `TranslationOnly` spellings are compatibility aliases for this
one behavior. A changed algorithm regenerates without an integrity grant;
replacing a modified, missing or disabled old **body** CRC requires explicit
`replace_modified` in the default profile. Ordinary pinned pcal's Noop callback
overwrites edited bodies; the TLC profile resolves that overwrite automatically,
and the flag is its explicit default-profile counterpart. Checksums remain a functional
feature; their presence is not a source-identity or model proof.

Translation, rewrite and original-AST reports use schema2. They retain real
acceptance/exit, complete-module analysis, artifacts, effective/requested options
and source declarations, and remove proof-only fields and duplicate option-context
dumps. A checksum warning names unavailability; it never changes accepted generated
TLA into an unsupported result. Exact direct procedure-CRC requests remain honestly
unsupported. See D-008's checksum/API continuation in the difference guides.


**Dependencies.** `dive-api` depends on `dive-pluscal`, and `dive-pluscal`
depends on `dive-syntax`. dive does not call Java at runtime. `dive-trace` stays
independent of the translator.

## Consequences

- New PlusCal support extends this pipeline and `docs/pluscal-translation.md`.
  It does not need a new ADR unless it changes a decision above.
- The translator's internal reference vectors and conservative reservation units
  are implementation details and may be simplified.
- Disk writes, the TLC map file format, exact pretty-printing widths and
  historical options remain open work, listed in the translation guide.

Supersedes: 0022, 0026, 0030, 0032, 0035, 0037, 0039, 0041, 0042, 0043, 0044,
0045, 0046, 0047, 0049, 0053, 0056, 0058, 0061, 0066, 0069, 0073, 0077, 0079.

Current PlusCal overwrite policy: the default API/CLI protects modified or
unverified existing bodies, with `--replace-modified` as its explicit overwrite
counterpart. `--compat=tlc` resolves that protection to ordinary primary Noop
overwrite automatically. Pure low-level `rewrite_document` retains its caller's
explicit policy. `requested_editing` records input settings, `editing` records
effective settings, and old body CRC/match data remains observable. Marker,
source-option, generation, module-analysis and quota refusals are unchanged.
The fresh exact modified-body primary control supports its own complete output
comparison; missing/disabled header variants do not inherit header byte-identity
credit from that control.
