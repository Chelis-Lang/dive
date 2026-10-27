---- MODULE ErCmLcpEmpty ----
\* LongestCommonPrefix of the empty set at x = 2: TLC 2283.
EXTENDS Naturals, SequencesExt
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == LongestCommonPrefix(IF x = 2 THEN {} ELSE {<<1>>}) = <<1>>
====
