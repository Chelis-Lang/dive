---- MODULE P_fair_two_members ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; process P \in {1,2} begin L:g:=g+1; E:skip; end process; fair process Q \in {3,4} begin M:g:=g+1; F:skip; end process; end algorithm; *)
====
