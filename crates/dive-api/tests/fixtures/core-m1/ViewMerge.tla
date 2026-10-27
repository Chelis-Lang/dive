---- MODULE ViewMerge ----
\* VIEW merges states that agree on x. A state outside the model is checked
\* without a lookup: <<0, 2>> has the view of the initial state, yet Inv,
\* which reads t, fails on it.
EXTENDS Naturals
VARIABLES x, t
Init == x = 0 /\ t = 0
Next == x' = 1 - x /\ t' = t + 1
View == <<x>>
Constr == t < 2
Inv == t # 2
====
