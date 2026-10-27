---- MODULE Pow0 ----
\* 0^0 is undefined in TLC (EC.TLC_MODULE_NULL_POWER_NULL). The pinned TLC
\* raises it on the first step: 1 distinct, 1 generated.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x ^ x
====
