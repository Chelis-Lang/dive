---- MODULE MultiExtra_C_mixed_member_init ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variable x \in {0,1}; { L: x:=1; LE: g:=x; }  process (Q =3) variable y \in {0,1}; { M: y:=2; ME: g:=y; }  process (R \in{4,5}) variable z \in {0,1}; { N: z:=3; NE: g:=z; } } *)
====
