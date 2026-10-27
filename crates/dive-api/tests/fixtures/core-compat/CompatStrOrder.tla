---- MODULE CompatStrOrder ----
\* TLC orders strings by intern token, so CHOOSE picks "zeta", met first;
\* the default profile orders them by text and picks "alpha" (D-010).
VARIABLE x
Init == x = CHOOSE s \in {"zeta", "alpha"} : TRUE
Next == x' = x
Inv == x # "zeta"
====
