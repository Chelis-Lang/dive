---- MODULE PrefixRawEmpty ----
EXTENDS Integers, FiniteSets, Sequences, SequencesExt, TLC, Functions, Bags, FiniteSetsExt
VARIABLE x
Init == x=0
Next == /\ x<2 /\ x' = x+1
Inv == LET s == {x,TRUE} p == {} \X s IN LongestCommonPrefix(p)= <<>>
====
