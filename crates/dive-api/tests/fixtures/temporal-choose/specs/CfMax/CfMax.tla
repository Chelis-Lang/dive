---- MODULE CfMax ----
EXTENDS Naturals
VARIABLE x
Init == x=FALSE
Next == x'=~x
Max(S) == CHOOSE b \in S : \A c \in S : b >= c
P == []<>(Max(IF x THEN {1,2} ELSE {3,4})=4)
====
