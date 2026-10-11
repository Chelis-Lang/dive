---- MODULE NativeCardinalityHolds ----
EXTENDS FiniteSets
VARIABLE x
Init == x = {1}
Next == UNCHANGED x
Use(F(_)) == []<>(F(x) = 1)
Live == Use(Cardinality)
====
