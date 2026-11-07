---- MODULE StepThunk ----
\* A step-mode thunk that reads the primed variables, forced inside the first
\* stage of a composition: it must be re-evaluated for each intermediate row.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Ok == x' # 1
A == x' \in {1, 2} /\ Ok
B == x' = (x + 1) % 3
Prop == [][(x' = 0) => (A \cdot B)]_x
Composed == A \cdot B
Inv == x \in {0, 1, 2}
====
