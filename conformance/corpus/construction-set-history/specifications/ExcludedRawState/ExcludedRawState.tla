---- MODULE ExcludedRawState ----
EXTENDS Integers
CONSTANT a
VARIABLES es,step
Init == /\ es={} /\ step=0
Next == /\ step=0 /\ step'=1 /\ es'={a,1}
C == step=0
Inv == step=0 \/ a \in es
====
