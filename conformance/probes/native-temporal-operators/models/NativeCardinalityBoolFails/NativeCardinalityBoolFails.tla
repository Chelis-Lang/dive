---- MODULE NativeCardinalityBoolFails ----
EXTENDS FiniteSets
VARIABLE x
Init == x = FALSE
Next == UNCHANGED x
Use(F(_)) == []<>(F(IF x THEN {1} ELSE {}) = 1)
Relay(G(_)) == Use(G)
Live == Relay(Cardinality)
====
