---- MODULE Choices ----
EXTENDS TLC
VARIABLES a,b
Action(P)==a'=P /\ b'=P
Init==a=0 /\ b=0
Next==Action(RandomElement({0,1}))
====
