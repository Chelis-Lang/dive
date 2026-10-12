---- MODULE OpCaptureHolds ----
EXTENDS TLC
VARIABLE x
Init == x=TRUE
Next == UNCHANGED x
Use(F(_)) == []<>F(x)
Capture(saved) == LET Local(v) == v = saved IN Use(Local)
Live == Capture(TRUE)
====
