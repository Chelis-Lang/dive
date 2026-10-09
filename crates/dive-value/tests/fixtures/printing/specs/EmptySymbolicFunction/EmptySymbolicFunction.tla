---------------- MODULE EmptySymbolicFunction ----------------
EXTENDS Naturals, TLC
ASSUME PrintT([[Nat -> {}] -> {1}])
VARIABLE x
Init == x = 0
Next == x' = x
=============================================================================
