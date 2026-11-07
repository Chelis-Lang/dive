---- MODULE NextReadCalls ----
EXTENDS Naturals
VARIABLE x
RECURSIVE G(_)
G(n) == IF n = 0 THEN x ELSE G(n-1)
RECURSIVE F(_)
F(n) == IF n = 0 THEN G(0)' ELSE F(n-1)
RECURSIVE H(_)
H(n) == IF n = 0 THEN x' ELSE H(n-1)
Op(a) == \/ (x' = 0 /\ a = 0) \/ (x' = 1 /\ a = 1)
f[n \in 0..1] == IF n = 0 THEN x' ELSE f[n-1]
NextFcn == Op(f[1])
Init == x = 0
InitOp(a) == \/ (x = 0 /\ a = 0) \/ (x = 1 /\ a = 1)
InitRec == InitOp(G(1))
Stay == UNCHANGED x
Next == Op(F(1))
NextDirect == Op(H(1))
====
