---- MODULE P_fair_opposed_plus_minus ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; fair+ process P = 1 begin L:+g:=1; E:skip; end process; fair+ process Q = 2 begin M:-g:=2; F:skip; end process; end algorithm; *)
====
