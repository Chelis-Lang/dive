---- MODULE RuntimeStrings ----
EXTENDS Naturals, TLC
VARIABLE n, s
Init == /\ n = 0 /\ s = "0"
Next == /\ n < 3 /\ n' = n + 1 /\ s' = ToString(n + 1)
Inv == n < 3
====
