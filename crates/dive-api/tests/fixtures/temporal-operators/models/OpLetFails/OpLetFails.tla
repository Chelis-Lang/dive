---- MODULE OpLetFails ----
EXTENDS TLC
VARIABLE x
Init == x=FALSE
Next == UNCHANGED x
Use(F(_)) == []<>F(x)
Live == LET Local(v) == v IN Use(Local)
====
