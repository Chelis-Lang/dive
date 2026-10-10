---- MODULE MultiExtra_C_mixed_plain ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variable x=0; { L: x:=1; LE: g:=x; }  process (Q =3) variable y=0; { M: y:=2; ME: g:=y; }  process (R \in{4,5}) variable z=0; { N: z:=3; NE: g:=z; } } *)
====
