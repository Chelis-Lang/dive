---- MODULE InstanceOpsInner ----
EXTENDS Naturals
VARIABLE y
RECURSIVE A(_), M(_,_), N(_)
A(n) == IF n=0 THEN y ELSE A(n-1)+1
M(n, delta) == IF n=0 THEN y ELSE N(n-1)+delta
N(n) == IF n=0 THEN y ELSE M(n-1,1)+1
====
