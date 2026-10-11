---- MODULE Snapshot ----
EXTENDS Integers,TLC
CONSTANT Token
VARIABLE x
Init == x=0
Next == x'=1-x
Unused == \A i \in 1..4 : i=i
Hook == TRUE
====
