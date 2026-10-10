---- MODULE MultiExtra_P_empty_record_field ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{} variable x=0; begin L: x:=[x |-> 1,y |-> 2,z |-> 3].x; LE: g:=x; end process;  process Q =3 variable y=0; begin M: y:=[x |-> 1,y |-> 2,z |-> 3].x; ME: g:=y; end process;  process R \in{} variable z=0; begin N: z:=[x |-> 1,y |-> 2,z |-> 3].x; NE: g:=z; end process; end algorithm; *)
====
