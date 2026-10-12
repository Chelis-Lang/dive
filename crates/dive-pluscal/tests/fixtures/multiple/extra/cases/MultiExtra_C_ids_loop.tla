---- MODULE MultiExtra_C_ids_loop ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: while (x<2) { x:=x+1; }; LE: g:=x; }  process (Q =2) variable y=0; { M: while (y<2) { y:=y+1; }; ME: g:=y; }  process (R =3) variable z=0; { N: while (z<2) { z:=z+1; }; NE: g:=z; } } *)
====
