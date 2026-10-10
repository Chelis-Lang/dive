---- MODULE MultiExtra_C_sets_labels ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variable x=0; { x:=1; LE: g:=x; }  process (Q \in{3,4}) variable y=0; { y:=1; ME: g:=y; }  process (R \in{5}) variable z=0; { z:=1; NE: g:=z; } } *)
====
