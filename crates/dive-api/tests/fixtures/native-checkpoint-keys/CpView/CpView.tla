---- MODULE CpView ----
EXTENDS Naturals
VARIABLES x, n
Init == x = 0 /\ n \in {0,1}
Next == x < 4 /\ x' = x+1 /\ n' \in {0,1}
View == x
Inv == x < 3
====
