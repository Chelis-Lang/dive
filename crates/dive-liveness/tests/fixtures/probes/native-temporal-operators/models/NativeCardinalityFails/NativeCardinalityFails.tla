---- MODULE NativeCardinalityFails ----
EXTENDS FiniteSets
VARIABLE x
Init == x = {1}
Next == UNCHANGED x
Use(F(_)) == []<>(F(x) = 2)
Live == Use(Cardinality)
====
