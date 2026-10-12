---- MODULE MultiExtra_C_empty_labels ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{}) variable x=0; { x:=1; LE: g:=x; }  process (Q =3) variable y=0; { y:=1; ME: g:=y; }  process (R \in{}) variable z=0; { z:=1; NE: g:=z; } } *)
====
