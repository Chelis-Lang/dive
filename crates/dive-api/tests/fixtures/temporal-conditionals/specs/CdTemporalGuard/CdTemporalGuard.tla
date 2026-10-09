---- MODULE CdTemporalGuard ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == IF []<>x THEN <>[]x ELSE <>[]~x
====
