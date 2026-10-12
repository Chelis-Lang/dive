---- MODULE MultiExtra_C_sets_loop ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variable x=0; { L: while (x<2) { x:=x+1; }; LE: g:=x; }  process (Q \in{3,4}) variable y=0; { M: while (y<2) { y:=y+1; }; ME: g:=y; }  process (R \in{5}) variable z=0; { N: while (z<2) { z:=z+1; }; NE: g:=z; } } *)
====
