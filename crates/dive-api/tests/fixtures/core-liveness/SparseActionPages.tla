---- MODULE SparseActionPages ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' \in 0..127
Tick == (x < 4 \/ x >= 120) /\ x' = (x + 1) % 128
Spec == Init /\ [][Next]_<<x>> /\ WF_<<x>>(Tick)
Within == []<>(x < 128)
Outside == <>(x = 128)
Missing == []<>(x = 129)
====
