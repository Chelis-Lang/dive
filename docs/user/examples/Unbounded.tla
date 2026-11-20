---- MODULE Unbounded ----
\* Init and Next draw from Nat, which TLC's simulator cannot enumerate.
EXTENDS Naturals
VARIABLE x
Init == x \in Nat
Next == \E n \in Nat : x' = x + n
Small == x < 1000
====
