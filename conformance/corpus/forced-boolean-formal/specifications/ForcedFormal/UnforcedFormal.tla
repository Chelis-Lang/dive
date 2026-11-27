---- MODULE UnforcedFormal ----
\* D-047 control: without the forcing comparison, TLC and dive both expand
\* the disjunction and emit two identical successors.
VARIABLES x
TypeInvariant == x \in {0}
Init == x = 0
Use(p) == /\ p
          /\ x' = x
Next == Use(x = 0 \/ x = 0)
====
