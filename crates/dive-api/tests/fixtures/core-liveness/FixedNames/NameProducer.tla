---- MODULE NameProducer ----
EXTENDS Naturals,TLC
VARIABLES n,s
Init == n=37 /\ s="start"
Next == n'=n /\ s'=ToString(n+1)
P == <>[](s="start")
====
