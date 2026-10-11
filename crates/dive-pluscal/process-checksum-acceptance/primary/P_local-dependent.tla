---- MODULE P_local_dependent ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P \in {1,2} variables z=0,w\in {z}; begin L:g:=w; E:skip; end process; end algorithm; *)
====
