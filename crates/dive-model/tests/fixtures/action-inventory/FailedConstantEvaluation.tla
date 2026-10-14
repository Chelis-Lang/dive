---- MODULE FailedConstantEvaluation ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Bad == 1 \div 0
Step == x' = x
Next == Bad \/ Step
====
