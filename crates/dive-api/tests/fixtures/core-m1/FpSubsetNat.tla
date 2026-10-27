---- MODULE FpSubsetNat ----
\* A successor holding SUBSET Nat, whose fingerprint enumerates Nat.
EXTENDS Integers, Sequences
VARIABLES x, y
Init == x = 0 /\ y = 0
Next == \/ y < 3 /\ x' = x /\ y' = y + 1
        \/ y = 2 /\ x' = SUBSET Nat /\ y' = y
====
