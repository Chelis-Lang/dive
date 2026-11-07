---- MODULE HoBuiltinImp ----
\* => and <=> passed as operator arguments (issue #851): the core applies
\* them as written in place, => short-circuiting. The reference engine
\* refuses them in this position.
EXTENDS Naturals
VARIABLE x
Ap2(op(_, _), a, b) == op(a, b)
Init == x = 0
Next == x' = (x + 1) % 4
Inv == /\ Ap2(=>, x > 5, 1 \in 1)
       /\ Ap2(=>, x < 5, x < 4)
       /\ Ap2(<=>, x > 5, FALSE)
       /\ Ap2(\equiv, TRUE, TRUE)
====
