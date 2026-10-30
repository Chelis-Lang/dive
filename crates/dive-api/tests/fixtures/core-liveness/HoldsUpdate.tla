---- MODULE HoldsUpdate ----
EXTENDS Naturals
VARIABLES x,y
Init == x = <<0,0>> /\ y=0
A == \E i \in 1..2 : x[i]=0 /\ x'=[x EXCEPT ![i]=1] /\ UNCHANGED y
B == UNCHANGED x /\ y'=1-y
Spec == Init /\ [][A \/ B]_<<x,y>> /\ WF_<<x,y>>(A)
P == <> (x = <<1,1>>)
Q == [] (y=0)
====
