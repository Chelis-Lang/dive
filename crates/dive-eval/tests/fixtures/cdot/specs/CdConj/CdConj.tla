---- MODULE CdConj ----
\* Probe: a conjunct after A \cdot B. TLC also evaluates it after A, against
\* the intermediate state, where x' = 1, so the step from 0 to 2 that the
\* formula allows is never generated and TLC reports a deadlock.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == ((x' = x + 1) \cdot (x' = x + 1)) /\ x' = 2
====
