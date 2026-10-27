---- MODULE FpInit ----
\* An initial state holding Nat.
EXTENDS Integers
VARIABLES x
Init == x \in {Nat}
Next == UNCHANGED x
====
