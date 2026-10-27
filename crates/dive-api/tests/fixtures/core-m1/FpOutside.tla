---- MODULE FpOutside ----
\* A state outside the model is never fingerprinted, so holding Nat there is no error.
EXTENDS Integers, Sequences
VARIABLES x, y
Init == x = 0 /\ y = 0
Next == \/ y < 3 /\ x' = x /\ y' = y + 1
        \/ y = 2 /\ x' = Nat /\ y' = y
Con == y < 2
====
