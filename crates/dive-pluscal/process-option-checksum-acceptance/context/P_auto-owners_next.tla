---- MODULE P_auto_owners_next ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P = 1 begin g:=1; g:=2; end process; process Q \in {2,3} begin g:=1; g:=2; end process; end algorithm; *)
====
