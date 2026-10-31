---- MODULE InstanceTuple ----
EXTENDS Integers
VARIABLE x
---- MODULE Inner ----
VARIABLE y
T==<<y>>
====
I==INSTANCE Inner WITH y<-x
Init==x=0
Next==x<2 /\ x'=x+1
Inv==I!T # [a|->x]
====
