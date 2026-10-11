# Bounded macro AST checksum study

This study preserves 348 source rows: the complete 240-row original macro and
whole-corpus selection, 16 comment-boundary rows, 16 expression-layout rows,
four exact old rewrite/API regressions, 12 grammar-owned end controls, six
generated-declaration ordering controls, 20 closing-line contexts, two unique
inline-definition negatives and 32 marker-priority/lexical controls. The pinned oracle is
`94d0c5070852b37b801bb8998c436383ab7ee37b`, jar SHA
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`.

`capture.py` invokes the clean primary translator before development-only
`SourceOptionProbe` parse/name-fixed/post-translation stage extraction.
It retains exact source/output bytes, stream bytes, adapter hashes, serialized
AST/CRC, SANY and configured Validator separately. The primary translator does
not delegate native functionality. No existing golden was edited.

`tests/macro_checksum.rs` compares all 348 dispositions. Its exact census is
235 SANY-valid raw AST matches, 24 SANY-invalid raw matches without admission
credit, 66 accounted reference rejections and 23 named native refusals. The original 240 primary rows
contain 28 translated/SANY-invalid outputs; the native subset does not credit
any of them. A raw checksum is not an artifact, exhaustive run or model proof.

The first native comparison retained a real Barriers comment-delimiter
mismatch, then paired C/P decoration controls justified a scoped embedded
selector repair. Original byte spans/source are retained. Standalone parsing
still requires EOF. The original trailing `notes` native Syntax failures remain archived; an exact
parser-consumed end refinement now matches those rows and eight further pinned
owned-end controls. Missing endings and unterminated quotes still reject.

Multiline actuals/body substitutions and substituted junction columns remain
unsupported. Definition/procedure/name-fixed AST serialization is unproved.
Actual Lock/Barriers sources remain whole-source observations; no general
corpus or family certification is implied. Fair-header-only CRC semantics and
source-option/old-block safety checks are unchanged.

See [the executable user guide](../../../docs/pluscal-macro-checksums.md),
[ADR 0049](../../../docs/decisions/0049-bounded-macro-checksum-layout.md), and the
independent raw/public/checksum/model reports retained with the final producer.

The first 576-profile public replay retained two real `Lock` forward-reference
failures. Generated declarations now precede trailing definitions while strict
module admission retains real unknown-name negatives. The two new SANY-invalid
unknown-name references receive raw-only evidence, never artifact credit.

The 588-profile corrected epoch passes every selected disposition, but its
separate closing-line probes expose standalone inline-definition and block-comment
acceptance gaps. Those sources/results remain archived. The final guard matches
`trans.java`'s ASCII trim/line-comment rule and inserts after the whole closing
line; formfeed primary artifacts remain SANY-invalid and uncredited.


The complete source5 epoch passed all 632 profile rows, 1,093 workspace tests,
CM1 21/VTS16 at widths 1/4/8, and the recorded model/witness controls. A separate
outside-selection review then demonstrated an existing main selector defect:
a later unfair marker overrode an earlier-line fair algorithm. Those successful
selection results remain qualified by the newly retained wrong-answer evidence.
The expanded selection adds 16 C/P fair/unfair order and LF/CRLF controls, plus
10 split/tab/space and malformed marker controls. Native selection now matches
pinned line-first search and its unfair-first priority only on a single line.
An independently named `MarkerOptionProbe` repairs the original development
adapter's same-line fair-first mistake; old classes/captures remain unchanged.
Clean `pcal.trans` output always decides primary selection and header checksums.

Six additional single-marker C/P split/tab/space sources preserve the intermediate
source6 named-refusal observation. Discovery and bounded owned-region selection
now use the same line-scoped marker function, rather than a stale exact-space
detector. Unclosed comments and malformed marker prefixes keep real diagnostics.

## Frozen final runtime and retained history

The expanded source7 replay accounts for all 348 raw inputs and 696 profile
requests: 454 admitted exact whole-module CST/SANY/Validator artifacts,
132 reference rejections, 64 invalid-primary dispositions without admission
credit, and 46 named refusals. All 1,094 workspace tests and the complete
36-stage gate pass. CM1's 21 models and the 16 ValidationTestSuite smoke
models agree at workers 1, 4 and 8. These results do not certify either family.

Twelve curated macro models yield 144 width/store controls, six independently
validated strict witnesses and 24 rejected mutants. Two whole-Lock finite
configurations yield 24 controls, one strict witness and four rejected mutants.
Ten executable CLI/Cargo guides, 12 checksum mutations, two whole-module
CST/SANY guide pairs, 80 closing-line and eight inline-boundary controls,
six forged full-schema reports in both profiles and four retained fair-CRC
weakness controls pass. All 350 primary/guide source hashes and 2,528 raw
stream hashes are read back independently from the selection manifests.

`receipt.json` indexes lossless final evidence and the original failed epochs.
`consumer-final/candidate7/producer.json` retains the actual uncommitted frozen
producer identity (`source_commit: null`); it is never retagged as the later
delivery commit. Selected production Rust/Cargo and guide inputs are archived
in `source.tar.gz`, preserving each original gzip source entry and hash map.
This selected archive is explicitly not a complete Git snapshot. Immutable
executables remain separately held under their recorded SHA-256; their hashes
in the archive do not claim that executable bytes are included in Git.
`verify-delivery.py` checks every indexed artifact and archived source entry,
and fails on production Rust/Cargo drift. Later documentation/receipt updates
are listed separately from that runtime source.

The inherited marker selector defect is tracked as #192. Its main attribution
uses exact unchanged selector Rust snippets from the shipped commit; the
source5 runtime is separately qualified, without claiming a replay of main.
The original four raw/eight public marker disagreements and source6's single-
marker refusal remain archived. Source3's Lock ordering failures, source4's
interrupted/wrong-stage gate, source5's complete 632-row success before the
outside-selection discovery, and all partial campaigns retain their original
identities. Archives under `failure-history/` preserve the exact reports and
streams; no earlier producer receives final-source credit.
