---- MODULE LvStarve ----
\* Two processes, and only p1's step is fair. <>(c[p2] = 1) is violated by a
\* behaviour in which only p1 moves, forever round its own cycle.
EXTENDS Naturals
CONSTANTS Procs, p1, p2
VARIABLE c
vars == <<c>>
Init == c = [p \in Procs |-> 0]
Step(p) == c' = [c EXCEPT ![p] = (@ + 1) % 2]
Next == \E p \in Procs : Step(p)
Spec == Init /\ [][Next]_vars /\ WF_vars(Step(p1))
Prop == <>(c[p2] = 1)
====
