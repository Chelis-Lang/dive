---- MODULE LetTuple ----
EXTENDS Integers
VARIABLE x
Init==x=0
Next==x<2 /\ x'=x+1
Inv==LET T==<<x>> IN T # [a|->x]
====
