---- MODULE P8ParallelDet ----
\* Probe: determinism across worker counts. Three counters advance by 1 or 2.
\* The bad state a = b = c = 30 is reachable by paths of 45 to 90 steps, so a
\* breadth-first search must report a 46-state trace (45 steps). A search that
\* is only approximately breadth-first can report a longer trace, and the
\* number of states explored before the violation can vary run to run.
EXTENDS Naturals
VARIABLES a, b, c
Init == a = 0 /\ b = 0 /\ c = 0
Next == \/ \E d \in {1, 2} : a + d <= 40 /\ a' = a + d /\ UNCHANGED <<b, c>>
        \/ \E d \in {1, 2} : b + d <= 40 /\ b' = b + d /\ UNCHANGED <<a, c>>
        \/ \E d \in {1, 2} : c + d <= 40 /\ c' = c + d /\ UNCHANGED <<a, b>>
Inv == ~(a = 30 /\ b = 30 /\ c = 30)
====
