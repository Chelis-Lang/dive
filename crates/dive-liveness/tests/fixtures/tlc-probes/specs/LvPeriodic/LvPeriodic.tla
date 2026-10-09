---- MODULE LvPeriodic ----
\* A liveness violation in a state space big enough for TLC's periodic
\* liveness check to run before the search ends, for comparing -lncheck final
\* with the default mode. From the initial state the system can enter a two-
\* state loop that never changes a, so []<>(a = N) is violated early; the
\* rest is an N by N grid.
EXTENDS Naturals
CONSTANT N
VARIABLES a, b, mode
vars == <<a, b, mode>>
Init == a = 0 /\ b = 0 /\ mode = "grid"
Grid == /\ mode = "grid"
        /\ \/ a < N /\ a' = a + 1 /\ b' = b
           \/ b < N /\ b' = b + 1 /\ a' = a
           \/ a = N /\ b = N /\ a' = 0 /\ b' = 0
        /\ UNCHANGED mode
Trap == mode = "grid" /\ a = 0 /\ b = 0 /\ mode' = "loop1" /\ UNCHANGED <<a, b>>
Loop == /\ mode \in {"loop1", "loop2"}
        /\ mode' = IF mode = "loop1" THEN "loop2" ELSE "loop1"
        /\ UNCHANGED <<a, b>>
Next == Grid \/ Trap \/ Loop
Spec == Init /\ [][Next]_vars /\ WF_vars(Next)
Prop == []<>(a = N)
====
