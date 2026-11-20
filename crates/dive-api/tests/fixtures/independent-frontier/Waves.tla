---- MODULE Waves ----
EXTENDS Naturals, Sequences
VARIABLE phase, path
Init == phase = 0 /\ path = <<>>
Next == \/ /\ phase = 0 /\ phase' = 1
           /\ \E n \in 1..512 : path' = <<n>>
        \/ /\ phase = 1 /\ phase' = 2 /\ path' = Append(path, 1)
        \/ /\ phase = 2 /\ UNCHANGED <<phase, path>>
Safe == ~(phase = 2 /\ path = <<483, 1>>)
TypeOK == phase \in 0..2
====
