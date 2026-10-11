---- MODULE P_two_constant ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P = 1 begin L:g:=g+1; E:skip; end process; process Q = 2 begin M:g:=g+1; F:skip; end process; end algorithm; *)
====
