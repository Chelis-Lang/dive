---- MODULE ErFoldOnSetDomain ----
\* FoldFunctionOnSet of an element outside the domain at x = 2.
EXTENDS Naturals, Functions
Sum(e, acc) == e + acc
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == FoldFunctionOnSet(Sum, 0, <<1, 2>>, IF x = 2 THEN {3} ELSE {1}) > 0
====
