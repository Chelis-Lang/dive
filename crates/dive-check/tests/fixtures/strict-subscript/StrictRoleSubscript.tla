---- MODULE StrictRoleSubscript ----
EXTENDS Naturals
VARIABLE x
R == INSTANCE StrictRolePart WITH c <- 1
Init == x=0
Next == x<2 /\ x'=x+1
Spec == Init /\ [][Next]_R!Sub
====
