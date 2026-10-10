---- MODULE MultiExtra_P_ids_with ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=0; begin L: with b0 \in {1,2} do x:=b0; end with; LE: g:=x; end process;  process Q =2 variable y=0; begin M: with b1 \in {1,2} do y:=b1; end with; ME: g:=y; end process;  process R =3 variable z=0; begin N: with b2 \in {1,2} do z:=b2; end with; NE: g:=z; end process; end algorithm; *)
====
