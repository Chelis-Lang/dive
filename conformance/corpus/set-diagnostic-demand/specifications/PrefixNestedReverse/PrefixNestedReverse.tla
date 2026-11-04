---- MODULE PrefixNestedReverse ----
EXTENDS Integers, FiniteSets, Sequences, SequencesExt, TLC, Functions
VARIABLE x
Init == x=0
Next == /\ x<2 /\ x' = x+1
Inv == LET s == {x, TRUE} p == (s \X {}) \X {x} IN LongestCommonPrefix(p)= <<>>
====
