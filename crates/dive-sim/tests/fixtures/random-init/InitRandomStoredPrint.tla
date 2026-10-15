---- MODULE InitRandomStoredPrint ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = RandomElement(IF PrintT("init-domain") THEN {2,7,7} ELSE {})
Next == x'=0 \/ x'=1
Safe == x>=0
====
