---- MODULE Missing ----
EXTENDS Integers, SVG
VARIABLE p
Init == p = PointOnLine([x |-> 0],[x |-> 8,y |-> 10],2)
Next == UNCHANGED p
====
