---- MODULE StoredGeneralHolds ----
EXTENDS FiniteSets
VARIABLE x
Init == x = [i \in {"alpha", "beta"} |-> {1}]
Next == UNCHANGED x
Live == []<>(Cardinality(x["alpha"]) = 1)
====
