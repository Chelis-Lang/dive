---- MODULE ErAliasOverflow ----
\* WP-C4 error-class matrix: 32-bit integer overflow in an ALIAS while the counterexample is printed.
EXTENDS Integers, TLC
VARIABLES x, y
Err(v) == v # 2 \/ 2147483647 + v > 0
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
Inv == x < 3
Alias == [x |-> x, y |-> y, e |-> Err(x)]
====
