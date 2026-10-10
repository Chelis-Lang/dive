---- MODULE MultiExtra_C_ids_labels ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { x:=1; LE: g:=x; }  process (Q =2) variable y=0; { y:=1; ME: g:=y; }  process (R =3) variable z=0; { z:=1; NE: g:=z; } } *)
====
