---- MODULE HoBuiltin ----
\* Built-in operators passed as operator arguments (issue #851), including
\* KeyValueStore's ReduceSet(\intersect, ...) through a LET-recursive
\* function. The unevaluated operands `1 \in 1` show that /\ and \/ keep
\* their short-circuit evaluation. `=>` and `<=>` are in
\* core-ho-builtin/HoBuiltinImp, which the reference engine refuses.
EXTENDS Naturals
VARIABLE x
Ap2(op(_, _), a, b) == op(a, b)
Ap1(op(_), a) == op(a)
Red(op(_, _), set, base) ==
  LET iter[s \in SUBSET set] ==
        IF s = {} THEN base
        ELSE LET y == CHOOSE y \in s : TRUE IN op(y, iter[s \ {y}])
  IN iter[set]
Inter(ss) == Red(\intersect, ss, UNION ss)
Init == x = 0
Next == /\ x' = (x + 1) % 4
        /\ Inter({{x, 1}, {x}})' = {x'}
        /\ Ap2(\cap, {x, 1}, {x})' = {x'}
Inv == /\ Inter({{1, 2, x}, {2, x}}) = {2, x}
       /\ Red(\cup, {{x}, {7}}, {}) = {x, 7}
       /\ Ap2(\cap, {1, 2}, {2, 3}) = {2}
       /\ Ap2(\cup, {1}, {x}) = {1, x}
       /\ Ap2(\union, {1}, {2}) = {1, 2}
       /\ Ap2(\, {1, 2}, {2}) = {1}
       /\ Ap2(=, x, x)
       /\ Ap2(/=, x, x + 1)
       /\ Ap2(#, x, x + 1)
       /\ Ap2(\in, x, 0..3)
       /\ Ap2(\notin, x, 4..5)
       /\ Ap2(\subseteq, {x}, 0..3)
       /\ Ap2(\land, TRUE, x < 4)
       /\ Ap2(\lor, FALSE, x < 4)
       /\ Ap2(/\, TRUE, TRUE)
       /\ Ap2(\/, FALSE, TRUE)
       /\ Ap2(\lor, TRUE, 1 \in 1)
       /\ Ap2(\land, FALSE, 1 \in 1) = FALSE
       /\ Ap1(\lnot, x > 5)
       /\ Ap1(~, FALSE)
       /\ Ap1(SUBSET, {x}) = {{}, {x}}
       /\ Ap1(UNION, {{x}, {1}}) = {x, 1}
       /\ Ap1(DOMAIN, <<x, x>>) = {1, 2}
====
