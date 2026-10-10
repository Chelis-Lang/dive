---- MODULE MultiExtra_C_sets_dependent_init ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variables x=0, xd=x+1; { L: x:=1; LE: g:=x; }  process (Q \in{3,4}) variables y=0, yd=y+1; { M: y:=2; ME: g:=y; }  process (R \in{5}) variables z=0, zd=z+1; { N: z:=3; NE: g:=z; } } *)
====
