---- MODULE RvBigIntvState ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0..2147483646
Next == UNCHANGED x
====
