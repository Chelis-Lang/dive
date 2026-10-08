---- MODULE EvRecFcn ----
EXTENDS Integers, Sequences, FiniteSets, TLC
fact[n \in 0..10] == IF n = 0 THEN 1 ELSE n * fact[n - 1]
fib[n \in Nat] == IF n < 2 THEN n ELSE fib[n - 1] + fib[n - 2]
sum[s \in SUBSET (1..4)] == IF s = {} THEN 0 ELSE LET x == CHOOSE y \in s : TRUE IN x + sum[s \ {x}]
ASSUME PrintT(fact[5])
ASSUME PrintT(fib[10])
ASSUME PrintT(sum[{1, 2, 3}])
ASSUME PrintT(fact[0])
ASSUME PrintT(DOMAIN fact)
====
