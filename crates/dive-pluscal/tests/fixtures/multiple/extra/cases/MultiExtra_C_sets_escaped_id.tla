---- MODULE MultiExtra_C_sets_escaped_id ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variable x=0; { L: x:=1; LE: g:=x; }  process (Q \in{3,4}) variable y=0; { M: y:=2; ME: g:=y; }  process (R ="a\tb") variable z=0; { N: z:=3; NE: g:=z; } } *)
====
