---- MODULE LvLeadsToHolds ----
\* As LvLeadsTo, but with SF_x(Go), where Go is the step from 1 to 3: it is
\* enabled infinitely often, so (x = 1) ~> (x = 3) holds.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Go == x = 1 /\ x' = 3
Next == \/ x = 0 /\ x' = 1
        \/ x = 1 /\ x' = 0
        \/ Go
        \/ x = 3 /\ x' = 3
Spec == Init /\ [][Next]_x /\ WF_x(Next) /\ SF_x(Go)
Prop == (x = 1) ~> (x = 3)
====
