---- MODULE GuardedAssume ----
EXTENDS Naturals, TLC
CONSTANT N
VARIABLE x

\* TLC checks the ASSUMEs in order and stops at the first that is not TRUE,
\* so the first guards the second, which is far too expensive to evaluate
\* when N is large.
ASSUME N <= 3
ASSUME \A i, j, k \in 1..(N * 1000) : i + j + k > 0

Init == x = 0
Next == x < 2 /\ x' = x + 1
====
