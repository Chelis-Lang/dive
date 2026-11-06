---- MODULE BindingEffectsDirect ----
EXTENDS Naturals, TLC
CONSTANT C
VARIABLE x
Target == IF PrintT("bound-target") THEN 5 ELSE 0
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
Inv == C+x = x+5 /\ C+x+10 = x+15
====
