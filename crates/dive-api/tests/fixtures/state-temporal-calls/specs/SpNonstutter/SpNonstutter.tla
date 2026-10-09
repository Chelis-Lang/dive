---- MODULE SpNonstutter ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live(a) == []<>a
Change(a) == []<><<a' # a>>_a
P == Change(x)
====
