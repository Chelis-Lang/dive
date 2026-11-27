---- MODULE ForcedFormal ----
\* D-047: the formal p is compared with TRUE before it is used as an action
\* conjunct. TLC then reuses the forced Boolean and emits one successor; dive
\* expands the disjunction both times, emitting two identical successors.
VARIABLES x
TypeInvariant == x \in {0}
Init == x = 0
Use(p) == /\ p = TRUE
          /\ p
          /\ x' = x
Next == Use(x = 0 \/ x = 0)
====
