---- MODULE InstanceOperatorParam ----
---- MODULE P ----
CONSTANT Op(_)
z == Op(1)
====
F(a) == a
I == INSTANCE P WITH Op <- F
====
