---------------- MODULE NonenumerableFunction ----------------
EXTENDS Naturals, TLC
ASSUME PrintT([[Nat -> {1}] -> {1}])
VARIABLE x
Init == x = 0
Next == x' = x
=============================================================================
