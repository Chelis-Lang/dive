---- MODULE Headers ----
EXTENDS Naturals
VARIABLE x,y
H( (* first line *)
 n ) == n+1
	Init(* header comment *) == /\ x=0 /\ y=0
A == /\ x<2 /\ x'=H(x) /\ y'=H(y)
B == /\ x=2 /\ x'=x /\ y'=y
Next == A \/ B
====
