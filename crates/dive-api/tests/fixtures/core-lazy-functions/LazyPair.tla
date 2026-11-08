---- MODULE LazyPair ----
\* Function constructors of several bound variables applied without being
\* built (TLC's `FcnLambdaValue.apply`): over `Nat`, and with entries whose
\* bodies would fail, which the application never evaluates.
EXTENDS Naturals
VARIABLE n
Add == [a \in Nat, b \in Nat |-> a + b]
Guarded[i, j \in 0..3] == IF i = 3 THEN 1 \div 0 ELSE i + j + n
Mixed[i \in 0..2, s \in {"x", "y"}, k \in 0..1] == IF s = "x" THEN i + k ELSE n
Init == n = 0
Next == n < 2 /\ n' = n + 1
Inv == /\ Add[n, 2] = n + 2
       /\ Guarded[n, 1] = 2 * n + 1
       /\ Mixed[n, "x", 1] = n + 1
       /\ Mixed[0, "y", 0] = n
Spec == Init /\ [][Next]_n
====
