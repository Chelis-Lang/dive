---- MODULE P_malformed ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P = 1 begin L:g:=; E:skip; end process; end algorithm; *)
====
