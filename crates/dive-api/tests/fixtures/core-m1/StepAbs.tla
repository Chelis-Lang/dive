---- MODULE StepAbs ----
\* The abstract specification StepCache and StepCacheBad instantiate; also
\* a fixture.
EXTENDS Naturals
VARIABLES tok, span, c
Init == tok = [pos |-> 1, sum |-> 0] /\ span = 0..0 /\ c = 0
Next == /\ tok.sum < 4
        /\ \E p \in 1..2 : tok' = [pos |-> p, sum |-> tok.sum + 1]
        /\ span' = 0..tok'.sum
        /\ c' = (c + 1) % 3
Spec == Init /\ [][Next]_<<tok, span, c>>
====
