---- MODULE MultiExtra_C_empty_member_init ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{}) variable x \in {0,1}; { L: x:=1; LE: g:=x; }  process (Q =3) variable y \in {0,1}; { M: y:=2; ME: g:=y; }  process (R \in{}) variable z \in {0,1}; { N: z:=3; NE: g:=z; } } *)
====
