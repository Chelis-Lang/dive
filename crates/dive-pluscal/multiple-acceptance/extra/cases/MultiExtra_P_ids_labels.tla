---- MODULE MultiExtra_P_ids_labels ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=0; begin x:=1; LE: g:=x; end process;  process Q =2 variable y=0; begin y:=1; ME: g:=y; end process;  process R =3 variable z=0; begin z:=1; NE: g:=z; end process; end algorithm; *)
====
