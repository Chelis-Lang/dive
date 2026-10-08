---- MODULE StutterNext ----
\* Probe (with StutterSpec): INIT/NEXT form. Next is disabled at x = 2.
\* Expected: 3 distinct, 3 generated (0, 1, 2), deadlock check off.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Spec == Init /\ [][Next]_x
====
