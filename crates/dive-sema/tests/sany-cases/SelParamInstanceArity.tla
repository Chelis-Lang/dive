---- MODULE SelParamInstanceArity ----
---- MODULE P ----
CONSTANT N
z == N
====
I(n) == INSTANCE P WITH N <- n
G == I!z
====
