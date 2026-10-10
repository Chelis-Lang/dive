---- MODULE MultiExtra_C_mixed_record_field ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variable x=0; { L: x:=[x |-> 1,y |-> 2,z |-> 3].x; LE: g:=x; }  process (Q =3) variable y=0; { M: y:=[x |-> 1,y |-> 2,z |-> 3].x; ME: g:=y; }  process (R \in{4,5}) variable z=0; { N: z:=[x |-> 1,y |-> 2,z |-> 3].x; NE: g:=z; } } *)
====
