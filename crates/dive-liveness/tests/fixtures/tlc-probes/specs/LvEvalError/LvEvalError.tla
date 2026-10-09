---- MODULE LvEvalError ----
\* The property's state predicate raises an evaluation error (division by
\* zero) on the initial state. What error class and exit code does TLC
\* report, and does it ever report success (compare upstream 35d40c91a)?
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == <>(10 \div x = 5)
====
