---- MODULE OpLetHolds ----
EXTENDS TLC
VARIABLE x
Init == x=TRUE
Next == UNCHANGED x
Use(F(_)) == []<>F(x)
Live == LET Local(v) == v IN Use(Local)
====
