---- MODULE MultiExtra_P_sets_loop ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=0; begin L: while x<2 do x:=x+1; end while; LE: g:=x; end process;  process Q \in{3,4} variable y=0; begin M: while y<2 do y:=y+1; end while; ME: g:=y; end process;  process R \in{5} variable z=0; begin N: while z<2 do z:=z+1; end while; NE: g:=z; end process; end algorithm; *)
====
