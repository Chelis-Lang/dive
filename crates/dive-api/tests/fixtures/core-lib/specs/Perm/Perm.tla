---- MODULE Perm ----
\* TLC!Permutations over integers and booleans, enumerated in actions.
EXTENDS Naturals, TLC
VARIABLE p
Init == p \in Permutations(1..3)
Next == \E q \in Permutations(DOMAIN p) : p' = [i \in DOMAIN p |-> p[q[i]]]
Inv == /\ Permutations({}) = {<<>>}
       /\ Permutations({TRUE, FALSE}) = {[b \in BOOLEAN |-> b], [b \in BOOLEAN |-> ~b]}
       /\ \A q \in Permutations(1..4) : {q[i] : i \in 1..4} = 1..4
       /\ (CHOOSE q \in Permutations(1..3) : TRUE) = <<1, 2, 3>>
====
