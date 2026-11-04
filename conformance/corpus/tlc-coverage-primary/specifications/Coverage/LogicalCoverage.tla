---- MODULE LogicalCoverage ----
EXTENDS Naturals, FiniteSets, TLC
VARIABLE x
Op1(S) == \A e \in SUBSET S : Cardinality(e) >= 0
Lazy(a,b) == a
Init == /\ x = 0
        /\ Lazy(Op1(1..3), Assert(FALSE, "unused"))
Op2(n) == CHOOSE i \in 1..n : \A j \in ((1..n) \ {i}) : i > j
A(n) == x' = Op2(n)
B(n) == x' = Op2(n)
Next == A(3) \/ B(2)
====
