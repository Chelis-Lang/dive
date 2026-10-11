---- MODULE P_modifiers_weak ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; fair+ process P \in {1,2} begin L:-g:=1; M:+g:=2; E:skip; end process; end algorithm; *)
====
