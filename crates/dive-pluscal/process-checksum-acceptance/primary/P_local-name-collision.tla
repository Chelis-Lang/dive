---- MODULE P_local_name_collision ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P = 1 variables z=0; begin L:g:=g+1; E:skip; end process; process Q = 2 variables z=0; begin M:z:=1; F:skip; end process; end algorithm; *)
====
