---- MODULE RgLevelViol ----
\* An invariant on TLCGet("level") fails: the trace is rebuilt at the
\* predecessors' levels.
EXTENDS Naturals, TLC
VARIABLE x
Init == x \in {0, 1}
Next == x' = x + TLCGet("level")
Inv == TLCGet("level") < 4
====
