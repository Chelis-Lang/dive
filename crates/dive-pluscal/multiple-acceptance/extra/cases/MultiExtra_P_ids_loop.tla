---- MODULE MultiExtra_P_ids_loop ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=0; begin L: while x<2 do x:=x+1; end while; LE: g:=x; end process;  process Q =2 variable y=0; begin M: while y<2 do y:=y+1; end while; ME: g:=y; end process;  process R =3 variable z=0; begin N: while z<2 do z:=z+1; end while; NE: g:=z; end process; end algorithm; *)
====
