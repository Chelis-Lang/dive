---- MODULE StartupFallback ----
EXTENDS Naturals
VARIABLE x

\* Deeper than the core's recursive-call limit: the binder's evaluation
\* supplies the start-up value, as it did before issue #846.
RECURSIVE SumTo(_)
SumTo(n) == IF n = 0 THEN 0 ELSE n + SumTo(n - 1)
Total == SumTo(80)

\* No candidate satisfies the predicate: the core frees each rejected one,
\* and neither evaluation gives a value, so the unreached use keeps the
\* body.
Unsat == CHOOSE r \in [1..5 -> 1..5] : \A n \in 1..5 : r[n] = n /\ r[n] # n

Init == x = 0
Next == x < 3 /\ x' = IF x = 99 THEN Unsat ELSE x + 1
Inv == x < Total
====
