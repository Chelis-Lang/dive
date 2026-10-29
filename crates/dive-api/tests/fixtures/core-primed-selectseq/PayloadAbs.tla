---- MODULE PayloadAbs ----
EXTENDS Integers
VARIABLE pending
Init == pending = 1
Next == \/ /\ pending = 1 /\ pending' = 0
        \/ /\ pending = 0 /\ pending' = 2
        \/ pending' = pending
vars == pending
Spec == Init /\ [][Next]_vars
====
