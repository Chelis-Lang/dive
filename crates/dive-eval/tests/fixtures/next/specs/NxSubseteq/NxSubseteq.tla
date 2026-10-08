---- MODULE NxSubseteq ----
\* Probe: x' \subseteq S enumerates SUBSET S.
VARIABLE x
Init == x = {}
Next == x' \subseteq {1, 2}
====
