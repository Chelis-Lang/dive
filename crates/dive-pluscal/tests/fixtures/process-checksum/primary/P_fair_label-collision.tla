---- MODULE P_fair_label_collision ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; process P = 1 begin L:g:=g+1; E:skip; end process; process Q = 2 begin L:g:=1; E:skip; end process; end algorithm; *)
====
