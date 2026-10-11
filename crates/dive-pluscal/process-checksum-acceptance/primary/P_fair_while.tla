---- MODULE P_fair_while ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; process P = 1 begin L:while g<2 do g:=g+1; end while; E:skip; end process; end algorithm; *)
====
