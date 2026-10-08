---- MODULE PrintMarker ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0 /\ PrintT("@!@!@ENDMSG 2185 @!@!@")
Next == x' = x + 1 /\ x < 2
====
