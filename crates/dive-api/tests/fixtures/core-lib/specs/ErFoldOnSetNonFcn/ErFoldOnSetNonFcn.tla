---- MODULE ErFoldOnSetNonFcn ----
\* FoldFunctionOnSet of a set as the function at x = 2: TLC 2169.
EXTENDS Naturals, Functions
Sum(e, acc) == e + acc
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == FoldFunctionOnSet(Sum, 0, IF x = 2 THEN {1} ELSE <<1>>, {1}) = 1
====
