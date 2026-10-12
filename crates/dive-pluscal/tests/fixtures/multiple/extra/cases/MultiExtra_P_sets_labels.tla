---- MODULE MultiExtra_P_sets_labels ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=0; begin x:=1; LE: g:=x; end process;  process Q \in{3,4} variable y=0; begin y:=1; ME: g:=y; end process;  process R \in{5} variable z=0; begin z:=1; NE: g:=z; end process; end algorithm; *)
====
