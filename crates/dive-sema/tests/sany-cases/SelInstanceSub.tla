---- MODULE SelInstanceSub ----
---- MODULE P ----
CONSTANT N
z == N + N
====
EXTENDS Naturals
I == INSTANCE P WITH N <- 1
G == I!z!1
====
