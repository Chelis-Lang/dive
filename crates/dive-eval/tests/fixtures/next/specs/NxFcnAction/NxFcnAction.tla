---- MODULE NxFcnAction ----
\* Probe: a lazily evaluated function applied as an action
\* (getFcnContext): Act[i] is enumerated in the function's context.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Act[i \in {1, 2}] == x' \in {x + i, x + i + 10} /\ x < 20
Next == \E i \in {1, 2} : Act[i]
====
