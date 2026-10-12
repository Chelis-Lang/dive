---- MODULE MultiExtra_C_ids_if ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: if (x=0) { x:=1; } else { x:=0; }; LE: g:=x; }  process (Q =2) variable y=0; { M: if (y=0) { y:=2; } else { y:=0; }; ME: g:=y; }  process (R =3) variable z=0; { N: if (z=0) { z:=3; } else { z:=0; }; NE: g:=z; } } *)
====
