---- MODULE RoActions ----
EXTENDS Naturals
CONSTANT Procs
VARIABLE pc
Init == pc = [p \in Procs |-> 0]
Step(p) == pc' = [pc EXCEPT ![p] = 1 - @]
Reset(p, q) == pc[p] = pc[q] /\ UNCHANGED pc
Next == \/ \E p \in Procs : Step(p)
        \/ \E p, q \in Procs : Reset(p, q)
        \/ \E p \in {} : Step(p)
        \/ TRUE
====
