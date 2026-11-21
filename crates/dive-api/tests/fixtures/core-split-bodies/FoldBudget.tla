---- MODULE FoldBudget ----
EXTENDS Naturals, FiniteSets
VARIABLE x
Init == x = 0
Step(i) ==
    LET n == Cardinality({k \in 1..4096 : k = 1})
    IN /\ i >= 0 /\ x' = n
Next == \E i \in 0..240 : Step(i)
====
