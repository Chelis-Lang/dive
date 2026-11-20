---- MODULE InfNext ----
\* Finite Init, but every state has infinitely many successors.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == \E n \in Nat : x' = x + n
Small == x < 1000000
====
