---- MODULE InstanceOperatorParamExpr ----
---- MODULE P ----
CONSTANT Op(_)
z == Op(1)
====
I == INSTANCE P WITH Op <- 1
====
