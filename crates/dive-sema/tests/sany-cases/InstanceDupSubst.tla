---- MODULE InstanceDupSubst ----
---- MODULE P ----
CONSTANT N
z == N
====
I == INSTANCE P WITH N <- 1, N <- 2
====
