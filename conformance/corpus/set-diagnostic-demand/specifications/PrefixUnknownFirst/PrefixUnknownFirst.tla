---- MODULE PrefixUnknownFirst ----
EXTENDS Integers, FiniteSets, Sequences, SequencesExt, TLC, Functions
VARIABLE x
Init == x=0
Next == /\ x<2 /\ x' = x+1
Inv == LongestCommonPrefix(x \X {})= <<>>
====
