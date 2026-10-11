---- MODULE C_modifiers_none_label ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; fair+ process(P \in {1,2}){L:-g:=1; M:+g:=2; E:skip;}} *)
====
