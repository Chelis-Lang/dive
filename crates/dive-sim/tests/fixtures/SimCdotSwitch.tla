---- MODULE SimCdotSwitch ----
\* Reads TLC's action composition switch, as FiniteMonotonic's MC modules do.
EXTENDS Naturals, TLC
ASSUME TLCGet("-Dtlc2.tool.impl.Tool.cdot") = "true"
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x
====
