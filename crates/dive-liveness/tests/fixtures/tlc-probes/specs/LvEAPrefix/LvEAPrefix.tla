---- MODULE LvEAPrefix ----
\* The violating behavior eventually stays where x > 0, after a prefix
\* where it is false. Pruning that prefix before reachability loses the
\* accepting component. An EA condition constrains the cycle, not the prefix.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == \/ x = 0 /\ x' = 1
        \/ x = 1 /\ x' = 2
        \/ x = 2 /\ x' = 1
Spec == Init /\ [][Next]_x
Prop == ~<>[](x > 0)
====
