---- MODULE DfidLive ----
EXTENDS Naturals
VARIABLES x
Init == x = 0
Next == x' = (x + 1) % 3
Live == []<>(x = 0)
====
