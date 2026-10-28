---- MODULE RgLevel ----
\* TLCGet("level") bounds an unbounded model: the checked state's level in
\* a constraint and an invariant, the expanded state's in the next-state
\* relation and an action constraint.
EXTENDS Naturals, TLC
VARIABLE x, y
Init == x = 0 /\ y = 0
Next == \/ x' = x + 1 /\ y' = TLCGet("level")
        \/ x' = x + 2 /\ UNCHANGED y
Constr == TLCGet("level") < 5
ActConstr == TLCGet("level") < 4 \/ x' # x + 2
Inv == y < TLCGet("level")
====
