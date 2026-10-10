# PlusCal process fairness

The native translator supports weak and strong fairness for its admitted
finite-process subset (including [multiple blocks](pluscal-multiple-processes.md)):

```tla
--algorithm A
variable result = 0;
fair process Worker \in {1, 2}
variable count = 0;
begin
  Step: + count := 1;
  Finish: - result := count;
end process;
end algorithm;
```

For this weak declaration, `Spec` quantifies weak fairness of the process action,
restricted by `pc[self] # "Finish"`, and strong fairness of `Step(self)`.
`fair + process` requests strong fairness of the guarded process action instead.
The modifiers follow the label colon: `Step: +`, not `Step+:`.

`dive translate FILE.tla -wf --output json` supplies weak fairness to a process
without a declaration; `-sf` supplies strong fairness. Explicit declarations
take precedence. `-wfNext` adds weak fairness of `Next`, while `-nof` suppresses
fairness in process syntax. A sequential `--fair algorithm` keeps its pinned
weak-`Next` behavior even with `-nof` or `-sf`. The JSON request limits record
the effective option selection. Both profiles share this translation contract.
Sequential fairness-option warnings are returned as structured data; full console
rendering remains outside artifact parity.

Label modifiers preserve the program counter in otherwise optimized infinite
loops. Process-local storage and source associations follow the
[single-process contract](pluscal-processes.md). Missing process labels still
require `--label`; fairness does not enable automatic insertion by default.

An accepted artifact passes complete enclosing-module analysis. It is not a
liveness verdict or a proof: `model_proved=false` and `exhaustive=false` remain
explicit. Native checking of the generated fairness formula is independently
limited by the checker's supported temporal families.

## Bounds and retained controls

The existing source/node/action/output bounds remain active. Multiple process
blocks, procedure/macro fairness, renaming, unproved domain normalization and
historical generation modes remain explicit refusals. Fair domains containing
newlines, tabs or comments are refused until the reference prefix transformation
is proved. Pretty-width, rewriting, checksums and map-file parity remain pending.

`crates/dive-pluscal/fairness-acceptance/` retains the original 150 selected
modifier-before-colon controls and a separate 150-row corrected selection.
Eight supplemental controls preserve long-label junction wrapping and the
multiline/comment/tab boundary. Every input, clean primary transcript, translated
module, SANY result and development-only normalization AST stays visible.
Invalid primary output never earns artifact credit. Another 48 sequential option
controls retain 16 original missing-C-body rejections and 32 valid artifacts,
including the source/option override warnings. Another 48 process-global fair
algorithm/option combinations prove that sequential-only warnings are not
invented for process syntax. The initial corrected process
capture with one five-second SANY timeout is retained in
`initial-positive-timeout`; the same 150 cases were freshly recaptured with an
explicit 60-second stage bound.

The reference's width estimate omits the sole quantifier-prefix line and the
conjunction separator. For example, the retained weak-process `L: +`/`E: -`
case prints an 87-column `Spec` line despite the default 78-column setting.
The native generator preserves the resulting conjunction grouping. Full-module
CST checks retain junction indentation, quantifier scope and prime scope; equal
significant tokens alone are insufficient evidence.

Run the retained scripts with a private cache and target, using the same pinned
Java/jar settings as the process corpus. `capture.py` is development-only;
`compare.py`, `validate.py` and `test-comparison.py` inspect the native artifact.
No Java tool is used by translation at runtime. The active 404-row study accounts for 246 artifacts, 148 matching rejections
and ten named refusals. Every artifact passes native enclosing-module analysis, separate pinned SANY
and full-module CST checks. Sixteen finite Init/Next model controls match clean TLC
at 1/4/8 workers with fingerprint/exact stores; four invariant witnesses pass
independent replay and sixteen corrupted witnesses are rejected. These action
controls do not certify liveness execution. All 21 general comparator corruptions and an invented sequential-only warning
are rejected. These curated controls leave F-034 and F-035 partial and audit-required.
