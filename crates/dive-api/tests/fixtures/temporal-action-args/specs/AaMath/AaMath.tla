---- MODULE AaMath ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x=0
Next == x'=1-x
Less(a,b) == a<b
P == []<><<Less(x,x')>>_x
====
