---- MODULE PropRefine ----
\* A refinement PROPERTY: an implied initial predicate and an implied action
\* through INSTANCE, checked on every step, to seen states and to states
\* outside the model too. It holds.
EXTENDS Naturals
VARIABLES x, y
Init == x = 0 /\ y = 0
Next == \/ x < 4 /\ x' = x + 1 /\ y' = y
        \/ y' = 1 - y /\ x' = x
Constr == x < 3
Abs == INSTANCE PropAbs WITH n <- x
AbsSpec == Abs!Spec
====
