---- MODULE InstanceConstLevelOk ----
---- MODULE P ----
CONSTANT N
z == N
====
VARIABLE v
I == INSTANCE P WITH N <- v
====
