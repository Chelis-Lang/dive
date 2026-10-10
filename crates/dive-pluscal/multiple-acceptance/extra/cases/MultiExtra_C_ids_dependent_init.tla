---- MODULE MultiExtra_C_ids_dependent_init ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variables x=0, xd=x+1; { L: x:=1; LE: g:=x; }  process (Q =2) variables y=0, yd=y+1; { M: y:=2; ME: g:=y; }  process (R =3) variables z=0, zd=z+1; { N: z:=3; NE: g:=z; } } *)
====
