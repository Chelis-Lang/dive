---- MODULE MultiExtra_C_empty_if ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{}) variable x=0; { L: if (x=0) { x:=1; } else { x:=0; }; LE: g:=x; }  process (Q =3) variable y=0; { M: if (y=0) { y:=2; } else { y:=0; }; ME: g:=y; }  process (R \in{}) variable z=0; { N: if (z=0) { z:=3; } else { z:=0; }; NE: g:=z; } } *)
====
