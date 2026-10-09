---- MODULE LtGuard ----
VARIABLE x
Init == x=FALSE
Live(p) == LET Now == p Stable(q) == <>[]q IN IF Now THEN Stable(p) ELSE Stable(~p)
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == Live(x)
====
