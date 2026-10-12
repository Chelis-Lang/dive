---- MODULE MultiExtra_C_ids_either ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: either { x:=1; } or { x:=2; }; LE: g:=x; }  process (Q =2) variable y=0; { M: either { y:=1; } or { y:=2; }; ME: g:=y; }  process (R =3) variable z=0; { N: either { z:=1; } or { z:=2; }; NE: g:=z; } } *)
====
