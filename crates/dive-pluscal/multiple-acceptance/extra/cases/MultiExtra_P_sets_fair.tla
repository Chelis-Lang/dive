---- MODULE MultiExtra_P_sets_fair ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0; fair process P \in{1,2} variable x=0; begin L: x:=1; LE: g:=x; end process; fair process Q \in{3,4} variable y=0; begin M: y:=2; ME: g:=y; end process; fair process R \in{5} variable z=0; begin N: z:=3; NE: g:=z; end process; end algorithm; *)
====
