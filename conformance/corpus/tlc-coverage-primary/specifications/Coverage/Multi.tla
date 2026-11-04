---- MODULE Multi ----
EXTENDS Naturals, TLC
VARIABLE x,y
H(n) == n+1
Init == /\ x=0 /\ y=0
A == /\ x<2 /\ x'=H(x) /\ y'=H(y)
B == /\ x=2 /\ x'=x /\ y'=y
Next == A \/ B
====
