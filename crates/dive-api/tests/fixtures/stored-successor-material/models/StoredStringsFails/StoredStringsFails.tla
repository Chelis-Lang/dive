---- MODULE StoredStringsFails ----

VARIABLE x
Init == x = {"alpha", "beta"}
Next == UNCHANGED x
Live == []<>(~("alpha" \in x))
====
