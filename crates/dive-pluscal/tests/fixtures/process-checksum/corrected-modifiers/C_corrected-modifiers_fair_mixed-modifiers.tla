---- MODULE C_corrected_modifiers_fair_mixed_modifiers ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; fair+ process(P = 1){L:-g:=1; M:+g:=2; E:skip;}} *)
====
