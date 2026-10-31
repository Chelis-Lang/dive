---- MODULE RecordLiteral ----
EXTENDS Integers
VARIABLE x
Init==x=0
Next==x<2 /\ x'=x+1
Inv==[a|->x] # <<x>>
====
