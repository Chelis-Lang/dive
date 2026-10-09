---- MODULE AaNested ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x=0
Next == x'=1-x
Less(a,b) == a<b
Relay(a,b) == Less(a,b)
P == []<><<Relay(x,x')>>_x
====
