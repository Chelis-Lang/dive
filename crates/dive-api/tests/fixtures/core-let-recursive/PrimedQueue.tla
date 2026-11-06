---- MODULE PrimedQueue ----
EXTENDS Naturals, Sequences
VARIABLES x, q
vars == <<x,q>>
Init == x=0 /\ q = << >>
Next == /\ x'=IF x=0 THEN 1 ELSE 0
        /\ q'=IF x=0 THEN <<3>> ELSE << >>
Fold == LET f[i \in 0..Len(q)] == IF i=0 THEN x ELSE f[i-1]+q[i]
        IN f[Len(q)]
Good == /\ Fold=x+(IF q = << >> THEN 0 ELSE q[1])
        /\ Fold'=x'+(IF q' = << >> THEN 0 ELSE q'[1])
Correct == [][Good]_vars
Wrong == [][Fold'=x]_vars
====
