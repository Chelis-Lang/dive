---- MODULE InfInit ----
\* Init draws x from Nat; Next draws a new value from Nat.
EXTENDS Naturals
VARIABLE x
Init == x \in Nat
Next == x' \in Nat
Small == x < 1000000
====
