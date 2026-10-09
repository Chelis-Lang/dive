---- MODULE LvPerProcessFair ----
\* Fairness quantified over processes, \A p \in Procs : WF_vars(Step(p)).
\* Each process counts to 2. <>(\A p : c[p] = 2) holds with per-process
\* fairness. Baseline for quantified fairness.
EXTENDS Naturals
CONSTANT Procs
VARIABLE c
vars == <<c>>
Init == c = [p \in Procs |-> 0]
Step(p) == c[p] < 2 /\ c' = [c EXCEPT ![p] = @ + 1]
Next == \E p \in Procs : Step(p)
Spec == Init /\ [][Next]_vars /\ \A p \in Procs : WF_vars(Step(p))
Prop == <>(\A p \in Procs : c[p] = 2)
====
