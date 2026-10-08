---- MODULE NxUnchangedChanged ----
\* Probe: UNCHANGED of a variable already assigned a different value
\* generates nothing (TLC prints TLC_UNCHANGED_VARIABLE_CHANGED as a
\* warning). B's successor exists, A's does not.
EXTENDS Naturals
VARIABLES x, y
Init == x = 0 /\ y = 0
A == x' = 1 /\ y' = 0 /\ UNCHANGED <<y, x>>
B == x' = 2 /\ UNCHANGED <<x, y>> /\ FALSE
C == x' = x /\ y' = y + 1 /\ y < 2 /\ UNCHANGED x
Next == A \/ B \/ C
====
