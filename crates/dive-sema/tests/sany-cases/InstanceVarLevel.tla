---- MODULE InstanceVarLevel ----
---- MODULE P ----
CONSTANT N
VARIABLE y
z == N
====
VARIABLE v
I == INSTANCE P WITH N <- v, y <- v
====
