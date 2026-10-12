---- MODULE MultiExtra_C_empty_loop ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{}) variable x=0; { L: while (x<2) { x:=x+1; }; LE: g:=x; }  process (Q =3) variable y=0; { M: while (y<2) { y:=y+1; }; ME: g:=y; }  process (R \in{}) variable z=0; { N: while (z<2) { z:=z+1; }; NE: g:=z; } } *)
====
