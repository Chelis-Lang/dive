---- MODULE CfLazy ----
EXTENDS Naturals
VARIABLE x
Init == x=FALSE
Next == x'=~x
H(S) == []<>((CHOOSE b \in S : TRUE)=TRUE)
P == H(IF x THEN {TRUE} ELSE {FALSE})
====
