#!/usr/bin/env python3
"""Retain every owned grammar case and curated sequential translation control."""
import hashlib
import json
from pathlib import Path


EXTRA = {
    "sequential_reads": "variables x=0, y=0; begin L: x := x+1; y := x; E: skip; end algorithm;",
    "parallel_swap": "variables x=1, y=2; begin L: x := y || y := x; E: skip; end algorithm;",
    "repeated_write": "variable x=0; begin L: x := 1; x := 2; end algorithm;",
    "empty_variables": "begin L: skip; E: skip; end algorithm;",
    "member_initial": "variable x \\in 0..2; begin L: x := x+1; E: skip; end algorithm;",
    "default_initial": "variable x; begin L: x := 1; E: skip; end algorithm;",
    "pc_conflict": "variable pc=0; begin L: pc := 1; E: skip; end algorithm;",
    "vars_conflict": "variable vars=0; begin L: vars := 1; E: skip; end algorithm;",
    "next_conflict": "variable Next=0; begin L: Next := 1; E: skip; end algorithm;",
    "label_conflict": "variable L=0; begin L: L := 1; E: skip; end algorithm;",
    "goto_done": "variable x=0; begin L: x := 1; goto Done; E: skip; end algorithm;",
    "infinite_loop": "variable x=0; begin L: while TRUE do x := (x+1) % 2; end while; end algorithm;",
    "finite_loop": "variable x=0; begin L: while x<2 do x := x+1; end while; E: skip; end algorithm;",
    "nested_labels": "variable x=0; begin L: while x<2 do B: x := x+1; end while; E: skip; end algorithm;",
    "condition_reads": "variables x=0, y=0; begin L: x := 1; if x=1 then y:=1; else y:=2; end if; E: skip; end algorithm;",
    "branch_unchanged": "variables x=0, y=0; begin L: if TRUE then x:=1; else y:=2; end if; E: skip; end algorithm;",
    "either_unchanged": "variables x=0, y=0; begin L: either x:=1; or y:=2; end either; E: skip; end algorithm;",
    "with_equal": "variable x=0; begin L: with k=2 do x:=k; end with; E: skip; end algorithm;",
    "with_member": "variable x=0; begin L: with k \\in {1,2} do x:=k; end with; E: skip; end algorithm;",
    "with_conflict": "variable x=0; begin L: with x=2 do skip; end with; E: skip; end algorithm;",
    "field_updates": "variable f=[a |-> 0, b |-> 0]; begin L: f.a:=1 || f.b:=2; E: skip; end algorithm;",
    "fair_algorithm": "variable x=0; begin L: x:=1; E: skip; end algorithm;",
    "goto_not_last": "variable x=0; begin L: goto E; x:=1; E: skip; end algorithm;",
    "assert_failure": "variable x=0; begin L: x:=1; assert x=0; E: skip; end algorithm;",
    "print_reads": "variable x=0; begin L: x:=1; print x; E: skip; end algorithm;",
    "await_reads": "variable x=0; begin L: x:=1; await x=1; E: skip; end algorithm;",
}

C_EXTRA = {
    "sequential_reads": "variables x=0, y=0; { L: x:=x+1; y:=x; E: skip; }",
    "parallel_swap": "variables x=1,y=2; { L: x:=y || y:=x; E: skip; }",
    "finite_loop": "variable x=0; { L: while (x<2) {x:=x+1;} E: skip; }",
    "infinite_loop": "variable x=0; { L: while (TRUE) {x:=(x+1)%2;} }",
    "nested_labels": "variable x=0; { L: while (x<2) { B: x:=x+1;} E: skip; }",
    "if": "variable x=0; { L: if (x=0) {x:=1;} else {x:=2;} E: skip; }",
    "elsif": "variable x=0; { L: if (x=0) {x:=1;} else if (x=1) {x:=2;} else {x:=3;} E: skip; }",
    "either": "variables x=0,y=0; { L: either {x:=1;} or {y:=2;} E: skip; }",
    "with_equal": "variable x=0; { L: with (k=2) {x:=k;} E: skip; }",
    "with_member": r"variable x=0; { L: with (k \in {1,2}) {x:=k;} E: skip; }",
    "fields": "variable f=[a|->0,b|->0]; { L: f.a:=1 || f.b:=2; E: skip; }",
    "assert": "variable x=0; { L: x:=1; assert x=0; E: skip; }",
    "await": "variable x=0; { L: x:=1; await x=1; E: skip; }",
    "print": "variable x=0; { L: x:=1; print x; E: skip; }",
    "goto": "variable x=0; { L: x:=1; goto E; E: goto Done; }",
    "empty": "{ L: skip; E: skip; }",
    "group": "variable x=0; { L: x:=1; {print x;} E: skip; }",
}
EXTRA.update({
    "elsif": "variable x=0; begin L: if x=0 then x:=1; elsif x=1 then x:=2; else x:=3; end if; E: skip; end algorithm;",
    "if_goto": "variable x=0; begin L: if x=0 then goto E; else skip; end if; E: skip; end algorithm;",
    "either_goto": "variable x=0; begin L: either goto E; or x:=1; end either; E: skip; end algorithm;",
    "with_goto": "variable x=0; begin L: with k=2 do goto E; end with; E: skip; end algorithm;",
    "indexed_update": "variable f=<<0,0>>; begin L: f[1]:=1; E: skip; end algorithm;",
    "record_field_keys": "variable x=0; begin L: x:=1; print [x|->x]; E: skip; end algorithm;",
    "comparison_rhs": "variable x=FALSE; begin L: x:=1=1; E: skip; end algorithm;",
    "nested_with": r"variable x=0; begin L: with a=1,b \in {1,2} do x:=a+b; end with; E: skip; end algorithm;",
    "label_plus": "variable x=0; begin L+: x:=1; E: skip; end algorithm;",
    "label_minus": "variable x=0; begin L-: x:=1; E: skip; end algorithm;",
    "quoted_done": 'variable x=0; begin L: x:=1; goto "Done"; end algorithm;',
    "done_label": "variable x=0; begin Done: x:=1; end algorithm;",
    "label_assert": "variable x=0; begin L: assert x=0; E: skip; end algorithm;",
    "unicode_forall": "variable x=0; begin L: x:=1; print ∀ k ∈ {1}: k=x; E: skip; end algorithm;",
    "unicode_exists": "variable x=0; begin L: x:=1; print ∃ k ∈ {1}: k=x; E: skip; end algorithm;",
    "unicode_function": "variable x=0; begin L: x:=1; print [k ∈ {1} ↦ k+x]; E: skip; end algorithm;",
    "unicode_record": "variable x=0; begin L: x:=1; print [x ↦ x]; E: skip; end algorithm;",
    "unicode_subscript": "variable f=[a ↦ <<0,0>>]; begin L: f.a[1]:=1; E: skip; end algorithm;",
    "multiline_junction": "variable x=0; begin L: print /\\ TRUE\n                            /\\ x=0; E: skip; end algorithm;",
    "parenthesized_true": "variable x=0; begin L: while (TRUE) do x:=(x+1)%2; end while; E: skip; end algorithm;",
})

def main():
    directory = Path(__file__).resolve().parent
    output = directory / "cases"
    output.mkdir(exist_ok=True)
    rows = []
    for case in sorted((directory.parent / "acceptance/cases").glob("*.pcal")):
        name = "Pc_" + case.stem.replace("-", "_")
        algorithm = case.read_text()
        source = f"---- MODULE {name} ----\nEXTENDS Integers, Sequences, FiniteSets, TLC\n(*\n{algorithm}\n*)\n====\n"
        (output / (name + ".tla")).write_text(source)
        rows.append({"id": name, "kind": "complete-owned-grammar-selection", "source": "crates/dive-pluscal/acceptance/cases/" + case.name,
                     "source_sha256": hashlib.sha256(case.read_bytes()).hexdigest()})
    correct_c = []
    complete_c = []
    terminated_c = []
    for key,body in C_EXTRA.items():
        before,rest = body.split("{",1)
        if before.strip():
            correct_c.append(("c_native_"+key, "{"+before+rest))
            complete_c.append(("c_current_"+key, "{"+before+"{"+rest+"}"))
            if key in ("either","elsif","finite_loop","group","if","nested_labels","with_equal","with_member"):
                terminated_c.append(("c_complete_"+key,"{"+before+"{"+rest.replace("} E:","}; E:")+"}"))
    for key, body in list(EXTRA.items()) + [("c_" + key, body) for key,body in C_EXTRA.items()] + correct_c + complete_c + terminated_c:
        name = "Pc_seq_" + key
        marker = "--fair algorithm A" if key == "fair_algorithm" else "--algorithm A"
        source = f"---- MODULE {name} ----\nEXTENDS Integers, Sequences, FiniteSets, TLC\n(*\n{marker}\n{body}\n*)\n====\n"
        (output / (name + ".tla")).write_text(source)
        rows.append({"id": name, "kind": "curated-sequential-control"})
    (directory / "selection.json").write_text(json.dumps({"schema_version": 1, "rows": rows}, indent=2) + "\n")
    print(f"Selected {len(rows)} complete translation inputs")


if __name__ == "__main__":
    main()
