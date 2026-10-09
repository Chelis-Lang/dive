---- MODULE LvWeakFair ----
\* With weak fairness on the whole next-state relation, x cycles 0, 1, 2
\* forever, so []<>(x = 0) holds. Baseline: what TLC prints for a liveness
\* success.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == []<>(x = 0)
====
