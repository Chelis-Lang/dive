---- MODULE MultiExtra_P_empty_labels ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{} variable x=0; begin x:=1; LE: g:=x; end process;  process Q =3 variable y=0; begin y:=1; ME: g:=y; end process;  process R \in{} variable z=0; begin z:=1; NE: g:=z; end process; end algorithm; *)
====
