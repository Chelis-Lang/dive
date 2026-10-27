---- MODULE FpSeqSet ----
\* A successor holding Seq({1}), which TLC cannot fingerprint.
EXTENDS Integers, Sequences
VARIABLES x, y
Init == x = 0 /\ y = 0
Next == \/ y < 3 /\ x' = x /\ y' = y + 1
        \/ y = 2 /\ x' = [a |-> Seq({1})] /\ y' = y
====
