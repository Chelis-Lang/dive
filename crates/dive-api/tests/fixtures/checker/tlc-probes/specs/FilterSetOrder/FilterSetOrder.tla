---- MODULE FilterSetOrder ----
\* Probe: in what order does x' \in {v \in S : P} enumerate? The filter walks
\* S with elements(), so it could keep the literal's source order (3 first)
\* or normalized order (1 first). The first successor ends the
\* counterexample. Observed: 1. MapSetOrder covers {e : v \in S}.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' \in {v \in {3, 1, 2} : v > 0}
Inv == x \notin {1, 3}
====
