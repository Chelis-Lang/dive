---- MODULE ConvertedEmptyProducts ----
EXTENDS Integers, Sequences, SequencesExt, Functions, Bags, FiniteSetsExt
CONSTANT Expected
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x + 1
Consumers == LET s == {x, TRUE} p == {} \X s IN
             /\ SetToBag(p) = <<>> /\ BagUnion(p) = <<>>
             /\ Quantify(p, LAMBDA z: TRUE) = 0 /\ x \in s
Inv == Consumers = Expected
====
