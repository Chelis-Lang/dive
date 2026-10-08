---- MODULE NxInitDup ----
\* Probe: initial states are generated with duplicates: x = 0 \/ x = 0
\* counts two generated initial states, one distinct.
VARIABLE x
Init == x = 0 \/ x = 0 \/ x = 1
Next == UNCHANGED x
====
