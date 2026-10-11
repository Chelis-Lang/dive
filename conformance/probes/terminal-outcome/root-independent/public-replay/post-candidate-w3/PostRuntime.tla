---- MODULE PostRuntime ----
EXTENDS Naturals,TLC
VARIABLE x
Init == x=0
Next == x'=1
Inv == x=0
Live == <>(x=2)
Post == x=x
====
