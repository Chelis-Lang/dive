---- MODULE CdOff ----
\* Probe: without the switch, TLCGet of it returns the key itself, and
\* A \cdot B is TLC's error.
EXTENDS Naturals, TLC
VARIABLE x
ASSUME TLCGet("-Dtlc2.tool.impl.Tool.cdot") = "-Dtlc2.tool.impl.Tool.cdot"
Init == x = 0
Next == (x' = x + 1) \cdot (x' = x + 1)
====
