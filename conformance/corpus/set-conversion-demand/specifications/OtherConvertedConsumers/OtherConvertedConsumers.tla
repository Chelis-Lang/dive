---- MODULE OtherConvertedConsumers ----
EXTENDS Integers, FiniteSets, Sequences, SequencesExt, TLC, Functions, Bags, FiniteSetsExt
VARIABLE x
Init == x=0
Next == /\ x<2 /\ x' = x+1
Inv == LET s == {x,TRUE} p == {} \X s IN
       /\ SetToBag(p)= <<>> /\ BagUnion(p)= <<>>
       /\ Quantify(p,LAMBDA z: TRUE)=0 /\ x \in s
====
