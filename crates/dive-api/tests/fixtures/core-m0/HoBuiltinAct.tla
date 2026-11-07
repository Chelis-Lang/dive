---- MODULE HoBuiltinAct ----
\* A built-in passed as an operator argument and applied as an action:
\* TLC's action splitting of it is not reproduced, so the core refuses.
EXTENDS Naturals
VARIABLE x
Ap2(op(_, _), a, b) == op(a, b)
Init == x = 0
Next == Ap2(\/, x' = (x + 1) % 3, x' = 0)
====
