---- MODULE SVG ----
EXTENDS Integers
PointOnLine(from,to,segment) == [x |-> from.x + ((to.x-from.x) \div segment),y |-> from.y + ((to.y-from.y) \div segment)]
====
