---- MODULE CfNested ----
EXTENDS Naturals
VARIABLE x
Init == x=FALSE
Next == x'=~x
P == []<>(\E i \in {x} : (CHOOSE b \in IF i THEN {TRUE} ELSE {FALSE} : b=i)=i)
====
