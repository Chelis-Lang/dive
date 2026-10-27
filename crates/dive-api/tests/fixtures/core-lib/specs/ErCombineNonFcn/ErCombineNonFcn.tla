---- MODULE ErCombineNonFcn ----
\* The first argument of @@ is not a function at x = 1: TLC 2169.
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == DOMAIN ((IF x = 1 THEN 7 ELSE <<1>>) @@ (2 :> 2)) # {}
====
