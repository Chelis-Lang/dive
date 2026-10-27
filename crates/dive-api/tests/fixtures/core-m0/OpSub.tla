---- MODULE OpSub ----
\* Operator declarations followed through nested instances (G <- F <- H),
\* down to a constant operator the configuration binds (H <- Inc), and an
\* action constant (Set <- Assign).
EXTENDS Integers
CONSTANTS H(_), Set(_)
VARIABLE x
Inc(v) == v + 1
Assign(v) == x' = v
M == INSTANCE OpSubMid WITH F <- H, A <- Set
Init == x = 0
Next == M!Inner!Step \/ (x = 2 /\ Set(H(H(x))))
Inv == x \in 0..5
====
