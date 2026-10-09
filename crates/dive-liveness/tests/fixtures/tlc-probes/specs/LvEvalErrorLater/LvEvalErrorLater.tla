---- MODULE LvEvalErrorLater ----
\* As LvEvalError, but the error only occurs at x = 2, a state reached in the
\* search, not an initial one.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == []<>(x = 0 \/ 10 \div (x - 2) = 5)
====
