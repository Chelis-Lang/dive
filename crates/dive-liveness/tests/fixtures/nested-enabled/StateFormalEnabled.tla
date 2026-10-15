---- MODULE NestedEnabled ----
EXTENDS Naturals
VARIABLE x
Init==x=0
Next==x'=1
Q==ENABLED (x'=1)
Guard(p)==p /\ TRUE
A==Guard(Q)
B==Guard(Guard(Q))
C==Guard(LET held==Q IN held)
D==LET held==Q IN Guard(held)
====
