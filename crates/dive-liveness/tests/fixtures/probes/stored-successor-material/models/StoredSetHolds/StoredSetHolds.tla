---- MODULE StoredSetHolds ----
EXTENDS FiniteSets
VARIABLE x
Init == x = {1}
Next == UNCHANGED x
Live == []<>(Cardinality(x) = 1)
====
