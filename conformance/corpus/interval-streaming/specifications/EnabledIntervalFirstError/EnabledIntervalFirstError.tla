---- MODULE EnabledIntervalFirstError ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x' = (IF ENABLED (x' \in 1..1000000000 /\ 1 \div 0=0) THEN 1 ELSE 2)
====
