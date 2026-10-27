---- MODULE FpRecordSet ----
\* A successor holding an infinite record set.
EXTENDS Integers, Sequences
VARIABLES x, y
Init == x = 0 /\ y = 0
Next == \/ y < 3 /\ x' = x /\ y' = y + 1
        \/ y = 2 /\ x' = [a : Nat, b : {1}] /\ y' = y
====
