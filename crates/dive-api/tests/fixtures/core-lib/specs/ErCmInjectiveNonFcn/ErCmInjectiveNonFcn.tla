---- MODULE ErCmInjectiveNonFcn ----
\* IsInjective of an integer at x = 1: TLC 2283.
EXTENDS Naturals, SequencesExt, Functions
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == IsInjective(IF x = 1 THEN 3 ELSE <<1>>)
====
