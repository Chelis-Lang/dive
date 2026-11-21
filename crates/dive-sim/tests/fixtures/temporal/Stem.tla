---- MODULE Stem ----
\* x goes 0 -> 1 -> 2 -> 1 -> 2 ...: a loop after a one-state stem.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x = 2 THEN 1 ELSE x + 1
Spec == Init /\ [][Next]_x
FairSpec == Spec /\ WF_x(Next)
EventuallyAlwaysPositive == <>[](x > 0)
====
