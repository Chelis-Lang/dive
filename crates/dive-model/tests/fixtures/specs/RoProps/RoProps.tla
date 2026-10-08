---- MODULE RoProps ----
EXTENDS Naturals
VARIABLE x
S == {7, 8}
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x
TypeOK == x \in 0..2
Small == x < 3
Prop ==
  /\ x = 0
  /\ []TypeOK
  /\ [][x' # x]_x
  /\ []<>(x = 0)
  /\ \A i \in S : [](x # i)
  /\ [](x < 5 /\ x >= 0)
Prop2 == []Small
====
