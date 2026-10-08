---- MODULE NxInLazyFilter ----
\* Probe: x' \in {f \in [D -> R] : P} over a set TLC cannot reduce: a lazy
\* SetPredValue enumerated in [D -> R]'s order. The invariant fails at the
\* first successor with f[1] = 1.
VARIABLE f
Init == f = [i \in {1, 2} |-> 0]
Next == f' \in {g \in [{1, 2} -> {0, 1}] : g[2] = 1 \/ g[1] = 1}
Inv == f[1] = 0
====
