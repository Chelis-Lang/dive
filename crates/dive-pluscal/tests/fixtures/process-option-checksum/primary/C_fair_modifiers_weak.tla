---- MODULE C_fair_modifiers_weak ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; fair+ process(P \in {1,2}){L:-g:=1; M:+g:=2; E:skip;}} *)
====
