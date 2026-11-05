---- MODULE MappedSubsetsViolation ----
EXTENDS Naturals
VARIABLE x
Init == x = 1
Next == /\ x = 1 /\ x' = 2
Inv == {{x} : i \in 1..1} \in SUBSET SUBSET {1}
====
