---- MODULE ErBagSqNonFcn ----
\* A \sqsubseteq operand that is not a function: TLC 2176, at x = 3.
EXTENDS Naturals, Bags
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == (IF x = 3 THEN 3 ELSE <<1>>) \sqsubseteq <<1>>
====
