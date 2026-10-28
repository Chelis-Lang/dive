---- MODULE StepCacheBad ----
\* StepCache with a substitution the abstract step does not allow once
\* the second counter reaches 2: the implied action is violated there,
\* with a cached definition read at both ends of the step.
EXTENDS Naturals
VARIABLES net, k
Init == net = [i \in 1..2 |-> 0] /\ k = 0
Next == \E i \in 1..2 : /\ net[i] < 2
                        /\ net' = [net EXCEPT ![i] = @ + 1]
                        /\ k' = (k + 1) % 3
tok == [pos |-> IF net[1] >= net[2] THEN 1 ELSE 2, sum |-> net[1] + net[2]]
span == 0..(net[1] + net[2])
cnt == IF net[2] = 2 THEN 5 ELSE k
Abs == INSTANCE StepAbs WITH tok <- tok, span <- span, c <- cnt
AbsSpec == Abs!Spec
====
