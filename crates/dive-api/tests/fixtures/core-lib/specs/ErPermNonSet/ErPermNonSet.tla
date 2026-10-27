---- MODULE ErPermNonSet ----
\* Permutations of a function at x = 1: TLC 2176.
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == Permutations(IF x = 1 THEN <<1, 2>> ELSE {1, 2}) # {}
====
