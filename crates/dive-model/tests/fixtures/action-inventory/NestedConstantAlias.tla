---- MODULE NestedConstantAlias ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Base == FALSE
Alias == Base
Step == x' = x
Next == Alias \/ Alias \/ Step
====
