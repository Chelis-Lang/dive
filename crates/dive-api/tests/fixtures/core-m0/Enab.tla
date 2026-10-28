---- MODULE Enab ----
\* ENABLED on the core (04-plan-compiler.md section 5.5): a successor of the
\* action, complete or not, from the current state.
EXTENDS Naturals
P == {1, 2, 3}
VARIABLES x, y, f
vars == <<x, y, f>>
Inc(p) == f[p] < 2 /\ f' = [f EXCEPT ![p] = f[p] + 1] /\ UNCHANGED <<x, y>>
OnlyX == x < 3 /\ x' = x + 1
Clash == x' = 1 /\ x' = 2
Pick == \E v \in 0..x : v > 1 /\ y' = v
Boxed == [x' = x + 1]_vars
Angle == <<UNCHANGED x>>_x
Step == /\ ENABLED OnlyX
        /\ x' = x + 1 /\ UNCHANGED <<y, f>>
Next == \/ \E p \in P : Inc(p)
        \/ Step
        \/ (ENABLED Pick /\ y' = y + 1 /\ y < 2 /\ UNCHANGED <<x, f>>)
Init == x = 0 /\ y = 0 /\ f = [p \in P |-> 0]
Inv == /\ (ENABLED OnlyX) = (x < 3)
       /\ ~ENABLED Clash
       /\ (ENABLED Pick) = (x > 1)
       /\ ENABLED Boxed
       /\ ~ENABLED Angle
       /\ \A p \in P : (ENABLED Inc(p)) = (f[p] < 2)
       /\ (\E p \in P : ENABLED Inc(p)) \/ x + y + f[1] + f[2] + f[3] < 100
       /\ ~(x = 3 /\ y = 2 /\ f[1] = 2 /\ f[2] = 2 /\ f[3] = 2)
====
