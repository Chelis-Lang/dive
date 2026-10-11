---- MODULE Snapshot ----
EXTENDS Integers,TLC
CONSTANT Token
VARIABLE x
Init == x=0
Next == x'=1-x
Danger == IF x=0 THEN PrintT("WITNESS_NOT_CACHED") ELSE TRUE
Hook == TRUE
====
