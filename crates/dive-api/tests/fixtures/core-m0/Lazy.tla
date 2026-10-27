---- MODULE Lazy ----
\* Laziness the core reproduces (`04-plan-compiler.md` §4.1): recursive
\* functions, lazy function constructors applied without being built,
\* membership in lazy sets over infinite domains, and recursive operators.
EXTENDS Naturals, FiniteSets
CONSTANT N
VARIABLE n, s
fact[k \in 0..N] == IF k = 0 THEN 1 ELSE k * fact[k - 1]
fib[k \in Nat] == IF k < 2 THEN k ELSE fib[k - 1] + fib[k - 2]
Dbl == [k \in Nat |-> 2 * k]
Evens == {k \in Nat : k % 2 = 0}
Odds == Nat \ Evens
RECURSIVE Sum(_)
Sum(S) == IF S = {} THEN 0 ELSE LET x == CHOOSE y \in S : TRUE IN x + Sum(S \ {x})
Init == n = 0 /\ s = {}
Next == /\ n < N
        /\ n' = n + 1
        /\ s' = s \cup {n}
Inv == /\ fact[n] >= 1
       /\ fib[n] <= fact[n] + 1
       /\ Dbl[n] \in Evens
       /\ Dbl[n] + 1 \in Odds
       /\ n \in DOMAIN Dbl
       /\ Sum(s) * 2 = n * (n - 1)
       /\ Cardinality(s) = n
Spec == Init /\ [][Next]_<<n, s>>
====
