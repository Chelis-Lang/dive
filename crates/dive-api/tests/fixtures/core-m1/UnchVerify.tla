---- MODULE UnchVerify ----
\* UNCHANGED of expressions that are not variables is verified
\* (`verifyUnchanged`); tuples, definitions and arguments are followed.
EXTENDS Naturals
VARIABLES x, y, r
vars == <<x, y>>
Keep(v) == UNCHANGED v
Init == x = 0 /\ y = 0 /\ r = [a |-> 0, b |-> 0]
A == x < 3 /\ x' = x + 1 /\ r' = [r EXCEPT !.b = x] /\ UNCHANGED <<y, r.a>>
B == y < 2 /\ y' = y + 1 /\ Keep(<<x, r>>)
C == r' = [r EXCEPT !.a = 1 - @] /\ UNCHANGED vars /\ UNCHANGED (x + y)
D == x' = x /\ r' = r /\ UNCHANGED <<<<y>>, r.b>>
Next == A \/ B \/ C \/ D
====
