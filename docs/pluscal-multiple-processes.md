# Multiple finite PlusCal processes

`dive translate FILE.tla --output json` now admits several finite process blocks
in current C/P syntax, including their own labels, locals and fairness settings:

```tla
--algorithm A
variable result = 0;
process Writer = 1
variable sent = 0;
begin
  Send: sent := 1;
  FinishWriter: result := sent;
end process;
fair process Reader \in {2, 3}
variable received = 0;
begin
  Read: received := 2;
  FinishReader: result := received;
end process;
end algorithm;
```

`sent` is scalar; `received` is indexed by `self`. Each action preserves the
other process's locals. `ProcSet` combines the declared domains, while the shared
`pc` function starts each ID at its own block's first label. Separate process
wrappers and fairness formulas retain source scope. The reference orders constant
wrappers before existential process-set actions in `Next`. Its program-counter
and terminating-stutter omission decisions are preserved, including mixed
finite/infinite bodies.

Names of processes, locals and labels must be globally unique in this slice.
Domains must be provably disjoint literal nonnegative `u64` integer/string IDs,
finite literal sets or integer intervals. Escaped string IDs, overlapping or
unproved domains, namespace rewrites and cross-process local references remain
named refusals. A jump outside the current process's labels is rejected. Record
fields that collide with another block's local spelling are conservatively
refused until scoped expression binding is proved. Procedures/macros, definition
blocks, branch-internal labels and historical generation options remain pending.

Both compatibility profiles share this source-admission contract. Existing
label/default-initialization and [fairness options](pluscal-fairness.md) apply.
Limits charge each normalized part before retaining it, across both the first and
forced-pc passes; global declarations are stored once. Total actions and generated
bytes remain bounded, and the normalized-node quota is charged across all blocks
before their results are stored. The source-associated
`artifact.processes` list includes every admitted block. Accepted artifacts pass
enclosing-module syntax/name/level analysis and explicitly report no exhaustive
search and no model proof. No Java is used at runtime, and no input is rewritten.

## Evidence and reproduction

The first **108** selected two-block cases contain **92** token artifacts,
**2** matching rejections and **14** named refusals, with no disagreements.
`crates/dive-pluscal/multiple-acceptance/` retains the full selection and raw
clean primary output, including the original adapter-flag failures. `extra/`
retains a separate **144** three-block control/storage/fairness selection: **106**
artifacts, **8** matching rejections and **30** named refusals. `bounds/` retains
**12** empty/reversed/overlapping domain and membership-initialization controls:
**8** artifacts and **4** named refusals. The complete active census is **264**
rows: **206** artifacts, **10** matching rejections and **48** named refusals. Capture uses
pinned `pcal.trans`; the normalized-AST adapter is development evidence only.
Primary translation and separate SANY acceptance are recorded independently.

Run `make_cases.py` / `make_extra.py` to reproduce inputs, then `capture.py` with
`--java`, `--javac`, `--jar`, `--inputs`, `--output`, `--reason` and a recorded
`--timeout`. Build the native `translate` example, run `compare.py` with
`--manifest`, `--inputs`, `--native`, `--output`, then `validate.py` with
`--java`, `--jar`, `--inputs`, `--fixtures`, `--report`, `--example`, `--native`
and `--output`. Run `test-comparison.py --native <example>` for corrupted-report
controls and `recheck-history.py --native <example>` for previous process matrices.
Run `verify.py --native <dive> --example <example>` to verify final report/binary,
generated source and raw capture digests.
All **206** admitted artifacts have matching enclosing-module operator CSTs and
separate SANY acceptance. The **60** curated finite model runs match clean TLC
verdicts/counts/depth at 1/4/8 workers in fingerprint/exact stores; **14** witnesses
validate independently and **56** corrupted witnesses are rejected. **28** full
metadata/proof/disposition corruption controls are rejected by the comparator.
The initial comparator accepted four forged dispositions; those failed controls
and its script digest are preserved before the strengthened checks. Previous
process/fairness selections are replayed without editing their original raw files.
Full token/CST/SANY validation is separate from curated finite-model and witness
checks. These selections do not certify all artifact or compilation families;
formatting, checksum/map/file-rewriting parity and full translator support remain
pending. See [ADR 0037](decisions/0037-pluscal-multiple-finite-processes.md).
