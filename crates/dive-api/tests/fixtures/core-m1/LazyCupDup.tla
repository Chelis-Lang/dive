---- MODULE LazyCupDup ----
\* x' \in a filter over a lazy union whose parts overlap: TLC lists the
\* shared element twice unless it has normalized the same set object
\* before; the core refuses.
VARIABLE r
Init == r = [a |-> 0]
Next == r' \in {s \in [a : {1}] \cup [a : {1, 2}] : TRUE}
====
