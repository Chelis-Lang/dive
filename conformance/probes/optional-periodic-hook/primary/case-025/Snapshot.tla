---- MODULE Snapshot ----
EXTENDS Integers,TLC
CONSTANT Token
VARIABLE x
Init == x = (1 \div 0)
Next == x'=1-x
Hook == TRUE
====
