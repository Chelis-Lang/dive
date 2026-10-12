---- MODULE StoredModelAtomsHolds ----
CONSTANTS a, b
VARIABLE x
Init == x = {a, b}
Next == UNCHANGED x
Live == []<>(a \in x)
====
