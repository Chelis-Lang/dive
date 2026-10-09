---- MODULE CdState ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Spec == Init /\ [][Next]_x
P == [](IF x THEN <>~x ELSE <>x)
====
