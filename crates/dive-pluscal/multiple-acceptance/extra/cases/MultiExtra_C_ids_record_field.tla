---- MODULE MultiExtra_C_ids_record_field ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: x:=[x |-> 1,y |-> 2,z |-> 3].x; LE: g:=x; }  process (Q =2) variable y=0; { M: y:=[x |-> 1,y |-> 2,z |-> 3].x; ME: g:=y; }  process (R =3) variable z=0; { N: z:=[x |-> 1,y |-> 2,z |-> 3].x; NE: g:=z; } } *)
====
