---- MODULE CdCaseOverlap ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == [](CASE TRUE -> <>x [] x -> <>~x [] OTHER -> FALSE)
====
