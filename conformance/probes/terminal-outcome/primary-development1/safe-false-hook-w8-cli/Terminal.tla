---- MODULE Terminal ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x'=IF x<512 THEN x+1 ELSE x
Inv == x=0
Live == <>(x=0)
Post == FALSE
Hook == FALSE
====
