---- MODULE MultiExtra_C_empty_dependent_init ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{}) variables x=0, xd=x+1; { L: x:=1; LE: g:=x; }  process (Q =3) variables y=0, yd=y+1; { M: y:=2; ME: g:=y; }  process (R \in{}) variables z=0, zd=z+1; { N: z:=3; NE: g:=z; } } *)
====
