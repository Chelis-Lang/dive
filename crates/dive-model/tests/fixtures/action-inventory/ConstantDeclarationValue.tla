---- MODULE ConstantDeclarationValue ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
CONSTANT C
Step == x' = x
Next == C \/ Step
====
