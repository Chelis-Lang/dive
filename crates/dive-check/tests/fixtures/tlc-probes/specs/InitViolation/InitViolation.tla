---- MODULE InitViolation ----
\* Probe: an invariant fails at the third of six initial states. TLC counts each
\* initial state as generated before checking it, and adds it to the state set
\* and the queue before the invariant is evaluated, then aborts initial-state
\* generation. Expected: 3 generated, 3 distinct, 3 left on the queue.
EXTENDS Naturals
VARIABLE x
Init == x \in 0..5
Next == UNCHANGED x
Inv == x # 2
====
