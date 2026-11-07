---- MODULE GuardedAssumeImpure ----
EXTENDS Naturals, TLC
CONSTANT N
VARIABLE x

\* As GuardedAssume, with a guard that draws: the core evaluates only the
\* pure ASSUMEs before it, and leaves the guard and what follows it to the
\* reference evaluator.
ASSUME N > 0
ASSUME RandomElement({N}) <= 3
ASSUME \A i, j, k \in 1..(N * 1000) : i + j + k > 0

Init == x = 0
Next == x < 2 /\ x' = x + 1
====
