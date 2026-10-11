---- MODULE Service ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == \E i \in 1..65536: x'=IF x=0 THEN 1 ELSE 1
Live == <>(x=2)
Hook == TRUE
====
