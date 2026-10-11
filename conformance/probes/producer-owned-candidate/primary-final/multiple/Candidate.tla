---- MODULE Candidate ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'=IF x=0 THEN 1 ELSE 1
Good == <>(x=1)
Bad == <>(x=2)
Hook == TRUE
====
