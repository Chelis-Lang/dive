---- MODULE CdDeadlock ----
\* Probe: from x = 0, A leads to x = 1, where B is not enabled, so A \cdot B
\* has no step: the state is a deadlock. TLC counts the intermediate state as
\* generated, which hides the deadlock.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == (x' = x + 1) \cdot (x < 1 /\ x' = x + 1)
====
