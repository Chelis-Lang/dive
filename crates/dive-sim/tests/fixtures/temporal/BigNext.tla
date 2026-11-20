---- MODULE BigNext ----
\* Finite but large successor sets: one million successors per state.
EXTENDS Naturals
VARIABLE x
Init == x \in 0..999999
Next == x' \in 0..999999
Small == x >= 0
====
