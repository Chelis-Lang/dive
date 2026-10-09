---- MODULE AaCaptured ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x=0
Next == x'=1-x
Less(a,b) == LET Before == a IN Before<b
P == []<><<Less(x,x')>>_x
====
