---- MODULE CfMin ----
EXTENDS Naturals
VARIABLE x
Init == x=FALSE
Next == x'=~x
Min(S) == CHOOSE b \in S : \A c \in S : b <= c
P == []<>(Min(IF x THEN {1,2} ELSE {3,4})=1)
====
