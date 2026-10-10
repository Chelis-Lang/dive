---- MODULE MultiExtra_C_sets_either ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variable x=0; { L: either { x:=1; } or { x:=2; }; LE: g:=x; }  process (Q \in{3,4}) variable y=0; { M: either { y:=1; } or { y:=2; }; ME: g:=y; }  process (R \in{5}) variable z=0; { N: either { z:=1; } or { z:=2; }; NE: g:=z; } } *)
====
