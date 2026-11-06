---- MODULE CanonicalEmptyKeyOne ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == UNCHANGED x
F == ((1..x) \X Nat) :> 11
Inv == F[{}] = 11
====
