---- MODULE TmRandomStartup ----
EXTENDS Naturals, TLC, Randomization
CONSTANT N
VARIABLE x
K == RandomElement(1..N)
S == RandomSetOfSubsets(2, 2, 1..N)
ASSUME PrintT(<<K, S, RandomElement(1..N)>>)
Init == x = K
Next == x < K + 3 /\ x' = x + RandomElement({1, 2})
Inv == x # K + 2
====
