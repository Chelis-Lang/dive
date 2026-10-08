---- MODULE NxUnassigned ----
\* Probe: a successor that leaves a variable unassigned is an error
\* (TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT), counted as generated.
EXTENDS Naturals
VARIABLES x, y
Init == x = 0 /\ y = 0
Next == x' = x + 1
====
