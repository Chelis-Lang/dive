---- MODULE NxInLazyCup ----
\* Probe: x' \in S \cup T where neither side is a set enumeration keeps the
\* union lazy (SetCupValue), whose elements() lists S's elements then T's,
\* duplicates included: [a |-> 1] is generated twice from each state.
VARIABLE r
Init == r = [a |-> 0]
Next == r' \in [a : {1}] \cup [a : {1, 2}]
====
