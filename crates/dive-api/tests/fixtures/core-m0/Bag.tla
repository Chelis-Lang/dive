---- MODULE Bag ----
\* Bags in the core: EmptyBag, SetToBag, BagToSet, BagIn and CopiesIn,
\* with BagsExt's BagAdd and BagRemove (plain definitions) over them.
EXTENDS Naturals, Bags
CONSTANT N
VARIABLE b
Add(B, x) == IF x \in DOMAIN B
             THEN [e \in DOMAIN B |-> IF e = x THEN B[e] + 1 ELSE B[e]]
             ELSE [e \in DOMAIN B \cup {x} |-> IF e = x THEN 1 ELSE B[e]]
Remove(B, x) == IF x \in DOMAIN B
                THEN IF B[x] = 1 THEN [e \in DOMAIN B \ {x} |-> B[e]]
                     ELSE [e \in DOMAIN B |-> IF e = x THEN B[e] - 1 ELSE B[e]]
                ELSE B
Init == b \in {EmptyBag, SetToBag({1}), SetToBag(1..2)}
Next == \/ \E x \in 1..N : CopiesIn(x, b) < 2 /\ b' = Add(b, x)
        \/ \E x \in BagToSet(b) : b' = Remove(b, x)
Inv == /\ \A x \in 1..N : BagIn(x, b) <=> CopiesIn(x, b) > 0
       /\ BagToSet(b) = DOMAIN b
       /\ BagIn(7, SetToBag({7}))
       /\ ~BagIn(8, EmptyBag)
       /\ CopiesIn([k |-> 1], SetToBag({[k |-> 1]})) = 1
Spec == Init /\ [][Next]_b
====
