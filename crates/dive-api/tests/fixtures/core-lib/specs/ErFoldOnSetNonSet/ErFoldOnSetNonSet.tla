---- MODULE ErFoldOnSetNonSet ----
\* FoldFunctionOnSet of a sequence as the set at x = 2: TLC 2169.
EXTENDS Naturals, Functions
Sum(e, acc) == e + acc
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == FoldFunctionOnSet(Sum, 0, <<1>>, IF x = 2 THEN <<1>> ELSE {1}) = 1
====
