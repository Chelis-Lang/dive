---- MODULE P_owners_next ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; fair+ process P = 1 begin L:-g:=1; M:+g:=2; E:skip; end process; fair process Q = 2 begin N:+g:=2; R:-g:=1; F:skip; end process; end algorithm; *)
====
