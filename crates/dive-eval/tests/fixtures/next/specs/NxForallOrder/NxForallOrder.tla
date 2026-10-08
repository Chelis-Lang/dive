---- MODULE NxForallOrder ----
\* Probe: the order in which \A x \in S : A visits its bindings. TLC
\* enumerates the first binding, then conses the others onto the
\* continuation, so the last is processed second (Tool.java:1230-1246).
\* From x = 0 the binding i = 1 yields x = 1; then, through its TRUE
\* disjunct, the next binding visited assigns x. TLC visits 3 before 2, so
\* the invariant fails after 3 generated states, not 4.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x = 0 /\ \A i \in {1, 2, 3} : x' = i \/ TRUE
Inv == x # 3
====
