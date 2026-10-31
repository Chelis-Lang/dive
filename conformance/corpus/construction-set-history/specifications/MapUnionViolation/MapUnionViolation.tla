---- MODULE MapUnionViolation ----
EXTENDS Integers, FiniteSets
CONSTANTS a, b, c
VARIABLES es, step
Init == /\ es = {} /\ step = 0
Next == /\ step < 2 /\ step'=step+1
        /\ LET newV == <<a,step+1>>
               delivered == {<<b,0>>,<<c,0>>}
           IN /\ es'=es \cup {<<newV,pv>>:pv \in delivered}
              /\ <<newV,<<b,0>>>> \in es'
Inv == step<2
====
