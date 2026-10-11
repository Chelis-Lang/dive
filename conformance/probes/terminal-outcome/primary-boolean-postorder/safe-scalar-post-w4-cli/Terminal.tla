---- MODULE Terminal ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x'=1
Live == <>(x=0)
Post == IF TRUE THEN 1 ELSE x
====
