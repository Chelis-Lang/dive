---- MODULE InstanceNonLeibniz ----
---- MODULE P ----
CONSTANT Op(_)
VARIABLE v
z == Op(v)
====
VARIABLE v
F(a) == ENABLED (a' = 1)
I == INSTANCE P WITH Op <- F
====
