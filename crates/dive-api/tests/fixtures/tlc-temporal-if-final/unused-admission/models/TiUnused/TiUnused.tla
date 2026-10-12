---- MODULE TiUnused ----
VARIABLE x
Gate(v) == IF x=0 THEN v ELSE TRUE
Unused == IF x=0 THEN <>(x=1) ELSE TRUE
Init == x=0
Next == UNCHANGED x
Live == []<>(x=1)
====
