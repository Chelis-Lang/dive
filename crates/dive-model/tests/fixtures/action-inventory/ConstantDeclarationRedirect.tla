---- MODULE ConstantDeclarationRedirect ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
CONSTANT C
Replacement == FALSE
Step == x' = x
Next == C \/ C \/ Step
====
