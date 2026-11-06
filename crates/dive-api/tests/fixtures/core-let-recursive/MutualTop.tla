---- MODULE MutualTop ----
EXTENDS Naturals
VARIABLE x
RECURSIVE A(_), B(_)
A(n) == IF n=0 THEN x ELSE B(n-1)+1
B(n) == IF n=0 THEN x ELSE A(n-1)+1
Init == x=0
Next == x'=IF x<2 THEN x+1 ELSE x
Good == A(2)=x+2 /\ B(3)=x+3
Wrong == Good /\ A(2)=2
Nested(a) == LET RECURSIVE C(_), D(_)
                 C(n) == IF n=0 THEN a ELSE D(n-1)+1
                 D(n) == IF n=0 THEN a ELSE C(n-1)+1
             IN C(2)
NestedCorrect == Nested(x)=x+2
PrimeCorrect == [][(A(2))'=x'+2]_x
RECURSIVE LoopA(_), LoopB(_)
LoopA(n) == IF n<0 THEN x ELSE LoopB(n)
LoopB(n) == IF n<0 THEN x ELSE LoopA(n)
LoopBody == LoopA(x)=0
RECURSIVE ArgA(_), ArgB(_)
ArgA(n) == IF n<0 THEN x ELSE ArgB(ArgA(n))
ArgB(n) == IF n<0 THEN x ELSE ArgA(n)
LoopArgument == ArgA(x)=0
====
