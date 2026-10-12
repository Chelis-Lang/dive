---- MODULE OpCaptureFails ----
EXTENDS TLC
VARIABLE x
Init == x=FALSE
Next == UNCHANGED x
Use(F(_)) == []<>F(x)
Capture(saved) == LET Local(v) == v = saved IN Use(Local)
Live == Capture(TRUE)
====
