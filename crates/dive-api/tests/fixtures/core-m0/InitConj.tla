---- MODULE InitConj ----
\* A specification whose initial predicate has several conjuncts, which
\* TLC conjoins: `y \in {0, 1}` assigns y, and `x > 1` filters x.
EXTENDS Naturals
VARIABLES x, y
vars == <<x, y>>
Init == x \in 0..3
Wrap(i) == i
Next == x' = 1 - x /\ UNCHANGED y
Spec == Wrap(Init) /\ (y \in {0, 1}) /\ x > 1 /\ [][Next]_vars
====
