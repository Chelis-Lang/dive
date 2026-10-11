---- MODULE P_fair_loop_weak ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; process P \in {1,2} begin while g<2 do g:=g+1; end while; end process; end algorithm; *)
====
