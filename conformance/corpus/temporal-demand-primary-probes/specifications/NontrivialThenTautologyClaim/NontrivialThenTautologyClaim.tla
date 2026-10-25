---- MODULE NontrivialThenTautologyClaim ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x' = 1 - x
Bad == IF x = 0 THEN 1 \div 0 = 0 ELSE TRUE
P == ([]<>Bad) => TRUE
Q == []<>(x = 0)
Spec == Init /\ [][Next]_x /\ WF_x(Next)
====
