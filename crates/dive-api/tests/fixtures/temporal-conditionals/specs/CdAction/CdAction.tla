---- MODULE CdAction ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Spec == Init /\ [][Next]_x
P == [](IF x' # x THEN <>x ELSE <>~x)
====
