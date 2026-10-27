---- MODULE ErCombineSecond ----
\* The second argument of @@ is not a function at x = 2: TLC 2169.
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == DOMAIN ((1 :> 1) @@ (IF x = 2 THEN {} ELSE <<1>>)) # {}
====
