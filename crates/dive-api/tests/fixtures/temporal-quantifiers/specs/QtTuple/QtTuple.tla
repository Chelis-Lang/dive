---- MODULE QtTuple ----
CONSTANT S
Pairs == {<<0,1>>,<<1,0>>}
VARIABLE x
Init == x = 0
Next == x' \in {0,1}
P == \A <<i,j>> \in S: []<>(x=i \/ x=j)
====
