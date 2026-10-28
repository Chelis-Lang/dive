---- MODULE StepCache ----
\* A refinement PROPERTY whose substitutions are definitions (a record and
\* an interval), used primed and unprimed: in a step predicate the core
\* evaluates each at most once per step and keys it on the variables it
\* reads, primed or not (`docs/developer/core/06-memoization.md` §2.3).
\* Steps reach seen states and states outside the model. It holds.
EXTENDS Naturals
VARIABLES net, k
Init == net = [i \in 1..2 |-> 0] /\ k = 0
Next == \E i \in 1..2 : /\ net[i] < 2
                        /\ net' = [net EXCEPT ![i] = @ + 1]
                        /\ k' = (k + 1) % 3
tok == [pos |-> IF net[1] >= net[2] THEN 1 ELSE 2, sum |-> net[1] + net[2]]
span == 0..(net[1] + net[2])
Constr == net[1] + net[2] < 4
Abs == INSTANCE StepAbs WITH tok <- tok, span <- span, c <- k
AbsSpec == Abs!Spec
====
