---- MODULE MultiExtra_C_sets_with ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variable x=0; { L: with (b0 \in {1,2}) { x:=b0; }; LE: g:=x; }  process (Q \in{3,4}) variable y=0; { M: with (b1 \in {1,2}) { y:=b1; }; ME: g:=y; }  process (R \in{5}) variable z=0; { N: with (b2 \in {1,2}) { z:=b2; }; NE: g:=z; } } *)
====
