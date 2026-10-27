---- MODULE OpSubInner ----
\* Applies the operator constant G, and the action constant Act.
EXTENDS Integers
CONSTANTS G(_), Act(_)
VARIABLE x
Step == x < 5 /\ Act(G(x))
====
