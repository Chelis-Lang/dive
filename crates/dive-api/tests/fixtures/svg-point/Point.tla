---- MODULE Point ----
EXTENDS Integers, SVG
VARIABLE p
Init == p = PointOnLine([x |-> -3,y |-> -1],[x |-> 2,y |-> 4],2)
Next == p' = [p EXCEPT !.y = @ + 1]
Inv == p.y = 1
====
